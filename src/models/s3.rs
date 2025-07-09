use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PolicyResponse {
    pub url: String,
    pub fields: PolicyFields,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PolicyFields {
    pub key: String,
    pub policy: String,
    #[serde(rename = "x-amz-algorithm")]
    pub x_amz_algorithm: String,
    #[serde(rename = "x-amz-credential")]
    pub x_amz_credential: String,
    #[serde(rename = "x-amz-date")]
    pub x_amz_date: String,
    #[serde(rename = "x-amz-signature")]
    pub x_amz_signature: String,
}
