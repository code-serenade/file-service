use serde::Deserialize;
use toolcraft_config::load_settings;
use toolcraft_jwt::VerifyJwt;
use toolcraft_request::{HeaderMap, Request};

use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub http: HttpCfg,
    pub jwt_verify: JwtVerifyRemoteCfg,
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
    pub endpoint: String,
    pub public_bucket: String,
    pub private_bucket: String,
    #[serde(default)]
    pub region: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    #[serde(default)]
    pub user_key_salt: Option<String>,
}

impl Settings {
    pub fn load(config_path: &str) -> Result<Self> {
        let r = load_settings(config_path)?;
        Ok(r)
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
        let verifier = VerifyJwt::new(
            payload.data.public_key_pem,
            payload.data.issuer,
            payload.data.audience,
        )?;

        Ok(verifier)
    }
}
