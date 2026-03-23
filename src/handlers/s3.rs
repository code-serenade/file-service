use std::sync::Arc;

use axum::{Extension, Json, extract::Query, http::StatusCode};
use sha2::{Digest, Sha256};
use toolcraft_axum_kit::{
    CommonError, CommonResponse, IntoCommonResponse, ResponseResult, middleware::auth_mw::AuthUser,
};
use toolcraft_utils::{presign_get_object, sign_request};
use uuid::Uuid;

use crate::{
    error::error_code,
    models::s3::{
        AccessSignQuery, DeleteSignQuery, DeleteSignResponse, DownloadSignResponse, UploadExtQuery,
        UploadHeaders, UploadSignResponse,
    },
    settings::S3Cfg,
    utils::base62::encode_u128,
};

#[utoipa::path(
    get,
    path = "/upload/avatar",
    responses(
        (status = 200, description = "Succeed", body = CommonResponse<UploadSignResponse>),
        (status = 500, description = "Error", body = CommonError)
    ),
    description = "获取头像上传签名",
    tag = "s3",
    security(("Bearer" = [])),
)]
pub async fn upload_avatar(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
) -> ResponseResult<UploadSignResponse> {
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = format!("avatars/{user_scope}");
    Ok(
        sign_upload_request(&s3, &s3.public_bucket, &key, None, Some("inline"))
            .into_common_response()
            .to_json(),
    )
}

#[utoipa::path(
    get,
    path = "/upload/image",
    params(
        ("ext" = String, Query, description = "图片后缀名，例如：jpg/jpeg/png/webp/gif")
    ),
    responses(
        (status = 200, description = "Succeed", body = CommonResponse<UploadSignResponse>),
        (status = 400, description = "Invalid params", body = CommonError),
        (status = 500, description = "Error", body = CommonError)
    ),
    description = "获取图片上传签名",
    tag = "s3",
    security(("Bearer" = [])),
)]
pub async fn upload_image(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
) -> ResponseResult<UploadSignResponse> {
    let ext = normalize_and_validate_ext(&query.ext, IMAGE_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let content_type = content_type_for_image_ext(&ext);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let object_id = encode_u128(Uuid::new_v4().as_u128());
    let key = format!("images/{user_scope}/{}.{}", object_id, ext);
    Ok(sign_upload_request(
        &s3,
        &s3.private_bucket,
        &key,
        Some(content_type),
        Some("inline"),
    )
    .into_common_response()
    .to_json())
}

#[utoipa::path(
    get,
    path = "/upload/document",
    params(
        ("ext" = String, Query, description = "文档后缀名，例如：pdf/doc/docx/xls/xlsx/txt")
    ),
    responses(
        (status = 200, description = "Succeed", body = CommonResponse<UploadSignResponse>),
        (status = 400, description = "Invalid params", body = CommonError),
        (status = 500, description = "Error", body = CommonError)
    ),
    description = "获取文档上传签名",
    tag = "s3",
    security(("Bearer" = [])),
)]
pub async fn upload_document(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
) -> ResponseResult<UploadSignResponse> {
    let ext = normalize_and_validate_ext(&query.ext, DOCUMENT_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let content_type = content_type_for_document_ext(&ext);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let object_id = encode_u128(Uuid::new_v4().as_u128());
    let key = format!("docs/{user_scope}/{}.{}", object_id, ext);
    Ok(sign_upload_request(
        &s3,
        &s3.private_bucket,
        &key,
        Some(content_type),
        Some("inline"),
    )
    .into_common_response()
    .to_json())
}

#[utoipa::path(
    get,
    path = "/access",
    params(
        ("key" = String, Query, description = "对象 key 或完整 URL（仅允许当前用户自己的私有资源）")
    ),
    responses(
        (status = 200, description = "Succeed", body = CommonResponse<DownloadSignResponse>),
        (status = 403, description = "Forbidden", body = CommonError),
        (status = 500, description = "Error", body = CommonError)
    ),
    description = "获取访问签名（仅当前用户私有资源）",
    tag = "s3",
    security(("Bearer" = [])),
)]
pub async fn access_sign(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<AccessSignQuery>,
) -> ResponseResult<DownloadSignResponse> {
    let normalized_key = normalize_object_key(&query.key, &s3.private_bucket).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;

    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    if !is_user_owned_private_key(&user_scope, &normalized_key) {
        return Err((StatusCode::FORBIDDEN, Json(error_code::FORBIDDEN.into())));
    }

    Ok(
        sign_access_request(&s3, &s3.private_bucket, &normalized_key)
            .into_common_response()
            .to_json(),
    )
}

#[utoipa::path(
    get,
    path = "/delete",
    params(
        ("key" = String, Query, description = "对象 key 或完整 URL（仅允许删除当前用户资源）")
    ),
    responses(
        (status = 200, description = "Succeed", body = CommonResponse<DeleteSignResponse>),
        (status = 400, description = "Invalid params", body = CommonError),
        (status = 403, description = "Forbidden", body = CommonError),
        (status = 500, description = "Error", body = CommonError)
    ),
    description = "获取删除签名（仅当前用户自己的资源）",
    tag = "s3",
    security(("Bearer" = [])),
)]
pub async fn delete_sign(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<DeleteSignQuery>,
) -> ResponseResult<DeleteSignResponse> {
    let (bucket, normalized_key) =
        normalize_object_key_with_bucket(&query.key, &s3.private_bucket, &s3.public_bucket).ok_or(
            (
                StatusCode::BAD_REQUEST,
                Json(error_code::INVALID_PARAMS.into()),
            ),
        )?;

    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    if !is_user_owned_key(&user_scope, &normalized_key) {
        return Err((StatusCode::FORBIDDEN, Json(error_code::FORBIDDEN.into())));
    }

    Ok(sign_delete_request(&s3, bucket, &normalized_key)
        .into_common_response()
        .to_json())
}

fn sign_upload_request(
    s3: &S3Cfg,
    bucket: &str,
    key: &str,
    content_type: Option<&str>,
    content_disposition: Option<&str>,
) -> UploadSignResponse {
    let host = s3
        .endpoint
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    let region = s3.region.as_deref().filter(|v| !v.trim().is_empty());

    let signed = sign_request(
        "PUT",
        &s3.access_key,
        &s3.secret_key,
        host,
        &format!("/{}/{}", bucket, key),
        "",
        region,
    );

    UploadSignResponse {
        method: "PUT".to_string(),
        upload_url: format!("{}/{}/{}", s3.endpoint.trim_end_matches('/'), bucket, key),
        key: key.to_string(),
        headers: UploadHeaders {
            authorization: signed.authorization,
            x_amz_date: signed.x_amz_date,
            x_amz_content_sha256: signed.x_amz_content_sha256,
            content_type: content_type.map(ToString::to_string),
            content_disposition: content_disposition.map(ToString::to_string),
        },
    }
}

fn sign_access_request(s3: &S3Cfg, bucket: &str, key: &str) -> DownloadSignResponse {
    let region = s3.region.as_deref().filter(|v| !v.trim().is_empty());

    let download_url = presign_get_object(
        &s3.access_key,
        &s3.secret_key,
        bucket,
        key,
        region,
        &s3.endpoint,
        Some(600),
    );

    DownloadSignResponse {
        method: "GET".to_string(),
        download_url,
        key: key.to_string(),
    }
}

fn sign_delete_request(s3: &S3Cfg, bucket: &str, key: &str) -> DeleteSignResponse {
    let host = s3
        .endpoint
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let region = s3.region.as_deref().filter(|v| !v.trim().is_empty());

    let signed = sign_request(
        "DELETE",
        &s3.access_key,
        &s3.secret_key,
        host,
        &format!("/{}/{}", bucket, key),
        "",
        region,
    );

    DeleteSignResponse {
        method: "DELETE".to_string(),
        delete_url: format!("{}/{}/{}", s3.endpoint.trim_end_matches('/'), bucket, key),
        key: key.to_string(),
        headers: UploadHeaders {
            authorization: signed.authorization,
            x_amz_date: signed.x_amz_date,
            x_amz_content_sha256: signed.x_amz_content_sha256,
            content_type: None,
            content_disposition: None,
        },
    }
}

fn is_user_owned_private_key(user_scope: &str, key: &str) -> bool {
    let image_prefix = format!("images/{user_scope}/");
    let doc_prefix = format!("docs/{user_scope}/");
    key.starts_with(&image_prefix) || key.starts_with(&doc_prefix)
}

fn is_user_owned_key(user_scope: &str, key: &str) -> bool {
    let avatar_key = format!("avatars/{user_scope}");
    if key == avatar_key {
        return true;
    }
    is_user_owned_private_key(user_scope, key)
}

const IMAGE_ALLOWED_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];
const DOCUMENT_ALLOWED_EXTS: &[&str] = &["pdf", "doc", "docx", "xls", "xlsx", "txt", "md"];

fn normalize_and_validate_ext(ext: &str, allowed: &[&str]) -> Option<String> {
    let normalized = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if allowed.contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_object_key(input: &str, private_bucket: &str) -> Option<String> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }

    let mut key = if let Some((_, after_scheme)) = raw.split_once("://") {
        let path = after_scheme.split_once('/').map(|(_, p)| p).unwrap_or("");
        path.trim_start_matches('/').to_string()
    } else {
        raw.trim_start_matches('/').to_string()
    };

    if let Some(stripped) = key.strip_prefix(&format!("{private_bucket}/")) {
        key = stripped.to_string();
    }

    if key.is_empty() { None } else { Some(key) }
}

fn normalize_object_key_with_bucket<'a>(
    input: &str,
    private_bucket: &'a str,
    public_bucket: &'a str,
) -> Option<(&'a str, String)> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }

    let mut path = if let Some((_, after_scheme)) = raw.split_once("://") {
        after_scheme
            .split_once('/')
            .map(|(_, p)| p)
            .unwrap_or("")
            .trim_start_matches('/')
            .to_string()
    } else {
        raw.trim_start_matches('/').to_string()
    };

    if let Some(stripped) = path.strip_prefix(&format!("{private_bucket}/")) {
        return Some((private_bucket, stripped.to_string()));
    }
    if let Some(stripped) = path.strip_prefix(&format!("{public_bucket}/")) {
        return Some((public_bucket, stripped.to_string()));
    }

    if path.is_empty() {
        return None;
    }

    if path.starts_with("avatars/") {
        Some((public_bucket, path))
    } else {
        Some((private_bucket, std::mem::take(&mut path)))
    }
}

fn user_scope_key(user_id: &str, s3: &S3Cfg) -> String {
    let salt = s3
        .user_key_salt
        .as_deref()
        .unwrap_or("change-me-user-key-salt");
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(user_id.as_bytes());
    let digest = hasher.finalize();
    let mut scope = String::with_capacity(32);
    for b in digest.iter().take(16) {
        scope.push_str(&format!("{b:02x}"));
    }
    scope
}

fn content_type_for_image_ext(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

fn content_type_for_document_ext(ext: &str) -> &'static str {
    match ext {
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    }
}
