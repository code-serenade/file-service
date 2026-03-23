use axum::{Router, routing::get};
use utoipa::OpenApi;

use crate::handlers::s3::{access_sign, delete_sign, upload_avatar, upload_document, upload_image};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::s3::upload_avatar,
        crate::handlers::s3::upload_image,
        crate::handlers::s3::upload_document,
        crate::handlers::s3::access_sign,
        crate::handlers::s3::delete_sign
    ),
    tags(
        (name = "s3", description = "S3 Upload APIs")
    ),
)]
pub struct S3Api;

pub fn s3_routes() -> Router {
    Router::new()
        .route("/upload/avatar", get(upload_avatar))
        .route("/upload/image", get(upload_image))
        .route("/upload/document", get(upload_document))
        .route("/access", get(access_sign))
        .route("/delete", get(delete_sign))
}
