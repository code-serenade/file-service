use axum::{Router, extract::DefaultBodyLimit, routing::get};

use crate::handlers::s3::{
    MAX_UPLOAD_BYTES, access_sign, delete_sign, put_upload_avatar, put_upload_document,
    put_upload_image, put_upload_media, upload_avatar, upload_document, upload_image, upload_media,
};

pub fn protected_routes() -> Router {
    Router::new()
        .route("/upload/avatar", get(upload_avatar).put(put_upload_avatar))
        .route("/upload/image", get(upload_image).put(put_upload_image))
        .route(
            "/upload/document",
            get(upload_document).put(put_upload_document),
        )
        .route("/upload/media", get(upload_media).put(put_upload_media))
        .route("/delete", get(delete_sign))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
}

pub fn access_routes() -> Router {
    Router::new().route("/access", get(access_sign))
}
