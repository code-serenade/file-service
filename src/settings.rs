use std::path::Path;

use serde::Deserialize;
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
struct S3KeysCfg {
    access_key: String,
    secret_key: String,
}

#[derive(Debug, Deserialize)]
pub struct S3Cfg {
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Browser-facing endpoint used when returning signed URLs to clients.
    pub endpoint: Option<String>,
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
    /// Per-resource settings. Credentials are overridden by config/keys.toml
    /// when that shared key file exists.
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub addressing_style: Option<S3AddressingStyle>,
    #[serde(default)]
    pub internal_endpoint: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub access_key: Option<String>,
    #[serde(default)]
    pub secret_key: Option<String>,
}

fn default_provider() -> String {
    "s3".to_string()
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum S3AddressingStyle {
    #[default]
    Path,
    VirtualHost,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ReadPolicy {
    Public,
    #[default]
    Private,
}

impl StorageBucketCfg {
    pub fn provider<'a>(&'a self, defaults: &'a S3Cfg) -> &'a str {
        self.provider.as_deref().unwrap_or(&defaults.provider)
    }

    pub fn endpoint<'a>(&'a self, defaults: &'a S3Cfg) -> &'a str {
        self.endpoint
            .as_deref()
            .or(defaults.endpoint.as_deref())
            .unwrap_or("")
    }

    pub fn addressing_style(&self, defaults: &S3Cfg) -> S3AddressingStyle {
        self.addressing_style.unwrap_or(defaults.addressing_style)
    }

    pub fn access_key(&self) -> &str {
        self.access_key.as_deref().unwrap_or("")
    }

    pub fn secret_key(&self) -> &str {
        self.secret_key.as_deref().unwrap_or("")
    }
}

impl Settings {
    pub fn load(config_path: &str) -> Result<Self> {
        let mut settings: Self = load_settings(config_path)?;
        let config_dir = Path::new(config_path)
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let keys_path = config_dir.join("keys.toml");
        if keys_path.exists() {
            let keys_path_str = keys_path.to_string_lossy();
            let keys: S3KeysCfg = load_settings(&keys_path_str)?;
            if keys.access_key.trim().is_empty() || keys.secret_key.trim().is_empty() {
                return Err(Error::Message(format!(
                    "{} requires non-empty access_key and secret_key",
                    keys_path.display()
                )));
            }
            for bucket in &mut settings.s3.buckets {
                bucket.access_key = Some(keys.access_key.clone());
                bucket.secret_key = Some(keys.secret_key.clone());
            }
        }
        Ok(settings)
    }

    pub fn validate_storage_buckets(&self) -> Result<()> {
        self.s3.validate_buckets("s3")?;
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
            if bucket.endpoint(self).trim().is_empty() {
                return Err(Error::Message(format!(
                    "[{provider_name}.buckets] '{}' has an empty endpoint",
                    bucket.id
                )));
            }
            if bucket.access_key().trim().is_empty() || bucket.secret_key().trim().is_empty() {
                return Err(Error::Message(format!(
                    "[{provider_name}.buckets] '{}' requires access_key and secret_key, or \
                     config/keys.toml",
                    bucket.id
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
}
