use serde::Deserialize;
use service_utils_rs::{services::jwt::JwtCfg, utils::load_settings};

use crate::error::Result;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub http: HttpCfg,
    pub jwt: JwtCfg,
    pub s3: S3Cfg,
}

#[derive(Debug, Deserialize)]
pub struct HttpCfg {
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct S3Cfg {
    pub endpoint: String,
    pub bucket_name: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

impl Settings {
    pub fn load(config_path: &str) -> Result<Self> {
        let r = load_settings(config_path)?;
        Ok(r)
    }
}
