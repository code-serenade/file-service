use std::sync::Arc;

use axum::{Extension, Json, extract::Query, http::StatusCode};
use toolcraft_axum_kit::{
    ApiError, IntoCommonResponse, ResponseResult, middleware::auth_mw::AuthUser,
};
use toolcraft_utils::{presign_get_object, sign_request};

use crate::{
    error::error_code,
    models::s3::{
        AccessSignQuery, DeleteSignQuery, DeleteSignResponse, DownloadSignResponse, UploadExtQuery,
        UploadHeaders, UploadSignResponse,
    },
    settings::S3Cfg,
    utils::base62::encode_u128,
};
use uuid::Uuid;

pub async fn upload_avatar(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
) -> ResponseResult<UploadSignResponse> {
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = format!("avatars/{user_scope}");
    Ok(
        sign_upload_request(&s3, &s3.public_bucket, &key, None, Some("inline"), false)
            .into_common_response()
            .to_json(),
    )
}

pub async fn upload_image(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
) -> ResponseResult<UploadSignResponse> {
    let ext = normalize_and_validate_ext(&query.ext, IMAGE_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let filename = normalize_and_validate_sha256_filename(query.filename.as_deref()).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let content_type = content_type_for_image_ext(&ext);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = format!("users/{user_scope}/images/{filename}.{ext}");
    let already_uploaded = object_exists(&s3, &s3.private_bucket, &key).await?;
    Ok(sign_upload_request(
        &s3,
        &s3.private_bucket,
        &key,
        Some(content_type),
        Some("inline"),
        already_uploaded,
    )
    .into_common_response()
    .to_json())
}

pub async fn upload_document(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
) -> ResponseResult<UploadSignResponse> {
    let ext = normalize_and_validate_ext(&query.ext, DOCUMENT_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let filename = normalize_and_validate_sha256_filename(query.filename.as_deref()).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let content_type = content_type_for_document_ext(&ext);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = format!("users/{user_scope}/docs/{filename}.{ext}");
    let already_uploaded = object_exists(&s3, &s3.private_bucket, &key).await?;
    Ok(sign_upload_request(
        &s3,
        &s3.private_bucket,
        &key,
        Some(content_type),
        Some("inline"),
        already_uploaded,
    )
    .into_common_response()
    .to_json())
}

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
    already_uploaded: bool,
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
        already_uploaded,
        headers: UploadHeaders {
            authorization: signed.authorization,
            x_amz_date: signed.x_amz_date,
            x_amz_content_sha256: signed.x_amz_content_sha256,
            content_type: content_type.map(ToString::to_string),
            content_disposition: content_disposition.map(ToString::to_string),
        },
    }
}

async fn object_exists(s3: &S3Cfg, bucket: &str, key: &str) -> Result<bool, ApiError> {
    let host = s3
        .endpoint
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let region = s3.region.as_deref().filter(|v| !v.trim().is_empty());

    let signed = sign_request(
        "HEAD",
        &s3.access_key,
        &s3.secret_key,
        host,
        &format!("/{}/{}", bucket, key),
        "",
        region,
    );
    let object_url = format!("{}/{}/{}", s3.endpoint.trim_end_matches('/'), bucket, key);
    let response = reqwest::Client::new()
        .head(object_url)
        .header("Authorization", signed.authorization)
        .header("x-amz-date", signed.x_amz_date)
        .header("x-amz-content-sha256", signed.x_amz_content_sha256)
        .send()
        .await
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;

    match response.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        StatusCode::FORBIDDEN => Err((StatusCode::FORBIDDEN, Json(error_code::FORBIDDEN.into()))),
        status if status.is_server_error() => Err((
            StatusCode::BAD_GATEWAY,
            Json(error_code::BAD_GATEWAY.into()),
        )),
        _ => Err((
            StatusCode::BAD_GATEWAY,
            Json(error_code::BAD_GATEWAY.into()),
        )),
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
    let image_prefix = format!("users/{user_scope}/images/");
    let doc_prefix = format!("users/{user_scope}/docs/");
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

fn normalize_and_validate_sha256_filename(filename: Option<&str>) -> Option<String> {
    let normalized = filename?
        .trim()
        .trim_end_matches(".sha256")
        .to_ascii_lowercase();
    if normalized.len() != 64 {
        return None;
    }
    if normalized.bytes().all(|b| b.is_ascii_hexdigit()) {
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
    let uuid = Uuid::parse_str(user_id).unwrap_or_else(|_| {
        let salt = s3.user_key_salt.as_deref().unwrap_or("change-me-user-key-salt");
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest as _;
        hasher.update(salt.as_bytes());
        hasher.update(b":");
        hasher.update(user_id.as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        Uuid::from_bytes(bytes)
    });
    encode_u128(uuid.as_u128())
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
