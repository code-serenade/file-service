use std::sync::Arc;

use axum::{Extension, Json, http::StatusCode};
use service_utils_rs::services::http::{
    CommonError, CommonResponse, IntoCommonResponse, response::ResponseResult,
};
use toolcraft::s3::generate_post_policy;

use crate::{error::error_code, models::s3::PolicyResponse, settings::S3Cfg};

#[utoipa::path(
    get,
    path = "/policy",
    responses(
        (status = 200, description = "Succeed", body = CommonResponse<PolicyResponse>),
        (status = 500, description = "Error", body = CommonError)
    ),
    description = "获取 S3 上传策略",
    tag = "s3",
    security(("Bearer" = [])),
)]
pub async fn policy(
    // Extension(UserId(user_id)): Extension<UserId>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    // Json(payload): Json<RunWorkflowRequest>,
) -> ResponseResult<PolicyResponse> {
    let value = generate_post_policy(
        &s3.access_key,
        &s3.secret_key,
        &s3.bucket_name,
        "upload/",
        &s3.region,
        &s3.endpoint,
        10,
    );
    let r1: PolicyResponse = serde_json::from_value(value).map_err(|_e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_code::SERVER_ERROR.into()),
        )
    })?;

    let res = r1.into_common_response().to_json();
    Ok(res)
}
