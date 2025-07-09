use axum::{Router, routing::get};
use utoipa::OpenApi;

use crate::handlers::s3::policy;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::s3::policy
    ),
    tags(
        (name = "s3", description = "S3 Management APIs")
    ),
)]

pub struct S3Api;

pub fn s3_routes() -> Router {
    Router::new().route("/policy", get(policy))
}
