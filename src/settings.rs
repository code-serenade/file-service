use serde::Deserialize;
use serde_json::json;
use toolcraft_config::load_settings;
use toolcraft_jwt::{VerifyJwt, VerifyJwtCfg};
use toolcraft_request::{HeaderMap, Request};

use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub http: HttpCfg,
    pub jwt_verify: JwtVerifyRemoteCfg,
    #[serde(default)]
    pub storage: StorageCfg,
    pub s3: S3Cfg,
    #[serde(default)]
    pub r2: Option<S3Cfg>,
}

#[derive(Debug, Deserialize)]
pub struct HttpCfg {
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct JwtVerifyRemoteCfg {
    pub url: String,
    pub header: String,
    pub token: String,
    #[serde(default)]
    pub avatar_provider_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct StorageCfg {
    #[serde(default)]
    pub user_key_salt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwtVerifyConfigResponse {
    code: i32,
    data: JwtVerifyConfigData,
    message: String,
}

#[derive(Debug, Deserialize)]
struct JwtVerifyConfigData {
    public_key_pem: String,
    issuer: String,
    audience: String,
}

#[derive(Debug, Deserialize)]
pub struct S3Cfg {
    #[serde(default)]
    pub provider: S3Provider,
    /// Browser-facing endpoint used when returning signed URLs to clients.
    pub endpoint: String,
    #[serde(default)]
    pub addressing_style: S3AddressingStyle,
    /// Optional endpoint for file-service's own S3 API requests.
    /// Falls back to `endpoint` to keep existing configurations compatible.
    #[serde(default)]
    pub internal_endpoint: Option<String>,
    #[serde(default)]
    pub buckets: Vec<StorageBucketCfg>,
    #[serde(default)]
    pub region: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    #[serde(default)]
    pub user_key_salt: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StorageBucketCfg {
    /// Logical storage category selected by file-service, never by a raw bucket name from clients.
    pub id: String,
    /// Provider bucket/container name.
    pub bucket: String,
    #[serde(default)]
    pub read_policy: ReadPolicy,
    #[serde(default)]
    pub key_prefix: String,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum S3Provider {
    #[default]
    S3,
    CloudflareR2,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum S3AddressingStyle {
    #[default]
    Path,
    VirtualHost,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadPolicy {
    Public,
    #[default]
    Private,
}

impl Settings {
    pub fn load(config_path: &str) -> Result<Self> {
        let r = load_settings(config_path)?;
        Ok(r)
    }

    pub fn validate_storage_buckets(&self) -> Result<()> {
        self.s3.validate_buckets("s3")?;
        if let Some(r2) = &self.r2 {
            r2.validate_buckets("r2")?;
        }
        Ok(())
    }
}

impl S3Cfg {
    fn validate_buckets(&self, provider_name: &str) -> Result<()> {
        const REQUIRED_BUCKET_IDS: [&str; 4] = ["avatar", "image", "document", "media"];

        for bucket in &self.buckets {
            if bucket.id.trim().is_empty() || bucket.bucket.trim().is_empty() {
                return Err(Error::Message(format!(
                    "[{provider_name}.buckets] requires non-empty id and bucket"
                )));
            }
            if self
                .buckets
                .iter()
                .filter(|candidate| candidate.id == bucket.id)
                .count()
                != 1
            {
                return Err(Error::Message(format!(
                    "[{provider_name}.buckets] has duplicate id '{}'",
                    bucket.id
                )));
            }
        }

        for bucket_id in REQUIRED_BUCKET_IDS {
            if !self.buckets.iter().any(|bucket| bucket.id == bucket_id) {
                return Err(Error::Message(format!(
                    "[{provider_name}.buckets] is missing required id '{bucket_id}'"
                )));
            }
        }

        Ok(())
    }
}

impl JwtVerifyRemoteCfg {
    pub async fn fetch_verify_jwt(&self) -> Result<VerifyJwt> {
        let client = Request::new()?;

        let mut headers = HeaderMap::new();
        headers.insert(self.header.clone(), self.token.clone())?;

        let response = client.get(&self.url, None, Some(headers)).await?;
        let status = response.status();
        let payload: JwtVerifyConfigResponse = response.json().await?;

        if !status.is_success() {
            return Err(Error::Message(format!(
                "fetch jwt verify config failed: status={}, message={}",
                status, payload.message
            )));
        }

        if payload.code != 0 {
            return Err(Error::Message(format!(
                "fetch jwt verify config failed: code={}, message={}",
                payload.code, payload.message
            )));
        }
        let verifier = VerifyJwt::new(VerifyJwtCfg {
            public_key_pem: payload.data.public_key_pem,
            issuer: payload.data.issuer,
            audience: payload.data.audience,
        })?;

        Ok(verifier)
    }

    pub async fn register_avatar_provider(&self, public_base_url: &str) -> Result<()> {
        let url = self
            .avatar_provider_url
            .as_deref()
            .ok_or_else(|| Error::Message("missing jwt_verify.avatar_provider_url".to_string()))?;
        let client = Request::new()?;
        let mut headers = HeaderMap::new();
        headers.insert(self.header.clone(), self.token.clone())?;
        let response = client
            .post(
                url,
                &json!({ "public_base_url": public_base_url }),
                Some(headers),
            )
            .await?;

        if !response.status().is_success() {
            return Err(Error::Message(format!(
                "register avatar provider failed: status={}",
                response.status()
            )));
        }
        Ok(())
    }
}

impl S3Cfg {
    pub fn avatar_public_base_url(&self) -> Result<String> {
        let bucket = self
            .buckets
            .iter()
            .find(|bucket| bucket.id == "avatar")
            .ok_or_else(|| Error::Message("missing avatar storage bucket".to_string()))?;

        if bucket.read_policy != ReadPolicy::Public {
            return Err(Error::Message(
                "avatar storage bucket must use public read_policy".to_string(),
            ));
        }

        let endpoint = self.endpoint.trim_end_matches('/');
        if self.addressing_style == S3AddressingStyle::VirtualHost {
            let (scheme, host) = endpoint.split_once("://").unwrap_or(("https", endpoint));
            Ok(format!("{scheme}://{}.{}", bucket.bucket, host))
        } else {
            Ok(format!("{endpoint}/{}", bucket.bucket))
        }
    }
}
