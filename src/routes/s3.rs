use axum::{Router, routing::get};

use crate::handlers::s3::{access_sign, delete_sign, upload_avatar, upload_document, upload_image};

pub fn s3_routes() -> Router {
    Router::new()
        .route("/upload/avatar", get(upload_avatar))
        .route("/upload/image", get(upload_image))
        .route("/upload/document", get(upload_document))
        .route("/access", get(access_sign))
        .route("/delete", get(delete_sign))
}
