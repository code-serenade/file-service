use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AccessSignQuery {
    pub key: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteSignQuery {
    pub key: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UploadExtQuery {
    pub ext: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DownloadSignResponse {
    pub method: String,
    pub download_url: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadSignResponse {
    pub method: String,
    pub upload_url: String,
    pub key: String,
    pub headers: UploadHeaders,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteSignResponse {
    pub method: String,
    pub delete_url: String,
    pub key: String,
    pub headers: UploadHeaders,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadHeaders {
    pub authorization: String,
    #[serde(rename = "x-amz-date")]
    pub x_amz_date: String,
    #[serde(rename = "x-amz-content-sha256")]
    pub x_amz_content_sha256: String,
    #[serde(rename = "Content-Type")]
    pub content_type: Option<String>,
    #[serde(rename = "Content-Disposition")]
    pub content_disposition: Option<String>,
}
