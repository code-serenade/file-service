use std::sync::Arc;

use axum::{
    Extension, Json,
    body::Bytes,
    extract::Query,
    http::{HeaderMap as AxumHeaderMap, StatusCode, header},
};
use sha2::Digest;
use toolcraft_axum_kit::{
    ApiError, IntoCommonResponse, ResponseResult, middleware::auth_mw::AuthUser,
};
use toolcraft_request::{HeaderMap, Request};
use toolcraft_utils::{presign_get_object, sign_request};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    error::error_code,
    models::s3::{
        AccessSignQuery, DeleteSignQuery, DeleteSignResponse, DownloadSignResponse,
        ServerUploadResponse, UploadAvatarQuery, UploadExtQuery, UploadHeaders, UploadSignResponse,
    },
    settings::{S3Cfg, S3Provider},
    utils::base62::encode_u128,
};

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

pub async fn put_upload_avatar(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadAvatarQuery>,
    headers: AxumHeaderMap,
    body: Bytes,
) -> ResponseResult<ServerUploadResponse> {
    let ext = normalize_and_validate_ext(&query.ext, IMAGE_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    validate_upload_body(&body)?;
    validate_image_payload(&ext, &body)?;

    let content_type = content_type_for_image_ext(&ext);
    validate_request_content_type(&headers, content_type)?;

    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = format!("avatars/{user_scope}");
    server_upload_object(&s3, &s3.public_bucket, &key, content_type, &body, false).await
}

pub async fn put_upload_image(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
    headers: AxumHeaderMap,
    body: Bytes,
) -> ResponseResult<ServerUploadResponse> {
    let ext = normalize_and_validate_ext(&query.ext, IMAGE_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    validate_upload_body(&body)?;
    validate_image_payload(&ext, &body)?;

    let content_type = content_type_for_image_ext(&ext);
    validate_request_content_type(&headers, content_type)?;

    let hash = sha256_hex(&body);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = format!("users/{user_scope}/images/{hash}.{ext}");
    server_upload_object(&s3, &s3.private_bucket, &key, content_type, &body, true).await
}

pub async fn put_upload_document(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
    headers: AxumHeaderMap,
    body: Bytes,
) -> ResponseResult<ServerUploadResponse> {
    let ext = normalize_and_validate_ext(&query.ext, DOCUMENT_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    validate_upload_body(&body)?;
    validate_document_payload(&ext, &body)?;

    let content_type = content_type_for_document_ext(&ext);
    validate_request_content_type(&headers, content_type)?;

    let hash = sha256_hex(&body);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = format!("users/{user_scope}/docs/{hash}.{ext}");
    server_upload_object(&s3, &s3.private_bucket, &key, content_type, &body, true).await
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
    let (bucket, normalized_key) = normalize_object_key_with_bucket(&query.key, &s3).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;

    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    if !is_user_owned_key(&user_scope, &normalized_key) {
        return Err((StatusCode::FORBIDDEN, Json(error_code::FORBIDDEN.into())));
    }

    Ok(sign_delete_request(&s3, bucket, &normalized_key)
        .into_common_response()
        .to_json())
}

async fn server_upload_object(
    s3: &S3Cfg,
    bucket: &str,
    key: &str,
    content_type: &str,
    body: &Bytes,
    check_existing: bool,
) -> ResponseResult<ServerUploadResponse> {
    let already_uploaded = if check_existing {
        object_exists(s3, bucket, key).await?
    } else {
        false
    };

    if !already_uploaded {
        put_object_to_s3(s3, bucket, key, content_type, body.clone()).await?;
    }

    Ok(ServerUploadResponse {
        key: key.to_string(),
        url: object_public_or_api_url(s3, bucket, key),
        sha256: sha256_hex(body),
        size: body.len(),
        already_uploaded,
    }
    .into_common_response()
    .to_json())
}

async fn put_object_to_s3(
    s3: &S3Cfg,
    bucket: &str,
    key: &str,
    content_type: &str,
    body: Bytes,
) -> Result<(), ApiError> {
    let host = s3_host(s3);
    let region = signing_region(s3);
    let signed = sign_request(
        "PUT",
        &s3.access_key,
        &s3.secret_key,
        &host,
        &format!("/{}/{}", bucket, key),
        "",
        region,
    );

    let object_url = s3_api_object_url(s3, bucket, key);
    let mut headers = HeaderMap::new();
    headers
        .insert("Authorization", signed.authorization)
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;
    headers
        .insert("x-amz-date", signed.x_amz_date)
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;
    headers
        .insert("x-amz-content-sha256", signed.x_amz_content_sha256)
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;
    headers
        .insert("Content-Type", content_type.to_string())
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;
    headers
        .insert("Content-Disposition", "inline".to_string())
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;

    let client = Request::new().map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            Json(error_code::BAD_GATEWAY.into()),
        )
    })?;
    let response = client
        .put_bytes(&object_url, body, Some(headers))
        .await
        .map_err(|err| {
            warn!(
                error = %err,
                bucket = %bucket,
                key = %key,
                object_url = %object_url,
                "s3 object PUT request failed before response"
            );
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;

    if response.status().is_success() {
        return Ok(());
    }

    warn!(
        status = response.status().as_u16(),
        bucket = %bucket,
        key = %key,
        object_url = %object_url,
        "s3 object PUT returned non-success status"
    );
    Err((
        StatusCode::BAD_GATEWAY,
        Json(error_code::BAD_GATEWAY.into()),
    ))
}

fn sign_upload_request(
    s3: &S3Cfg,
    bucket: &str,
    key: &str,
    content_type: Option<&str>,
    content_disposition: Option<&str>,
    already_uploaded: bool,
) -> UploadSignResponse {
    let host = s3_host(s3);

    let region = signing_region(s3);

    let signed = sign_request(
        "PUT",
        &s3.access_key,
        &s3.secret_key,
        &host,
        &format!("/{}/{}", bucket, key),
        "",
        region,
    );

    UploadSignResponse {
        method: "PUT".to_string(),
        upload_url: s3_api_object_url(s3, bucket, key),
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

fn s3_host(s3: &S3Cfg) -> String {
    s3.endpoint
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string()
}

fn s3_api_object_url(s3: &S3Cfg, bucket: &str, key: &str) -> String {
    format!("{}/{}/{}", s3.endpoint.trim_end_matches('/'), bucket, key)
}

fn object_public_or_api_url(s3: &S3Cfg, bucket: &str, key: &str) -> String {
    if bucket == s3.public_bucket {
        if let Some(public_bucket_url) = normalized_public_bucket_url(s3) {
            return format!("{}/{}", public_bucket_url, key);
        }
    }
    if bucket == s3.private_bucket {
        if let Some(private_bucket_url) = normalized_private_bucket_url(s3) {
            return format!("{}/{}", private_bucket_url, key);
        }
    }
    s3_api_object_url(s3, bucket, key)
}

fn signing_region(s3: &S3Cfg) -> Option<&str> {
    s3.region
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| (s3.provider == S3Provider::CloudflareR2).then_some("auto"))
}

fn normalized_public_bucket_url(s3: &S3Cfg) -> Option<&str> {
    s3.public_bucket_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(|url| url.trim_end_matches('/'))
}

fn normalized_private_bucket_url(s3: &S3Cfg) -> Option<&str> {
    s3.private_bucket_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(|url| url.trim_end_matches('/'))
}

async fn object_exists(s3: &S3Cfg, bucket: &str, key: &str) -> Result<bool, ApiError> {
    let host = s3_host(s3);
    let region = signing_region(s3);

    let signed = sign_request(
        "HEAD",
        &s3.access_key,
        &s3.secret_key,
        &host,
        &format!("/{}/{}", bucket, key),
        "",
        region,
    );
    let mut headers = HeaderMap::new();
    headers
        .insert("Authorization", signed.authorization)
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;
    headers
        .insert("x-amz-date", signed.x_amz_date)
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;
    headers
        .insert("x-amz-content-sha256", signed.x_amz_content_sha256)
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;

    let object_url = s3_api_object_url(s3, bucket, key);
    debug!(
        endpoint = %s3.endpoint,
        host = %host,
        region = ?region,
        bucket = %bucket,
        key = %key,
        object_url = %object_url,
        "checking s3 object existence"
    );

    let client = Request::new().map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            Json(error_code::BAD_GATEWAY.into()),
        )
    })?;
    let response = client
        .head(&object_url, Some(headers))
        .await
        .map_err(|err| {
            warn!(
                error = %err,
                error_debug = ?err,
                bucket = %bucket,
                key = %key,
                object_url = %object_url,
                "s3 object HEAD request failed before response"
            );
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })?;

    debug!(
        status = response.status().as_u16(),
        bucket = %bucket,
        key = %key,
        object_url = %object_url,
        "s3 object HEAD response received"
    );

    match response.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        StatusCode::FORBIDDEN => {
            debug!(
                bucket = %bucket,
                key = %key,
                object_url = %object_url,
                "s3 object HEAD returned forbidden; continue upload signing as not uploaded"
            );
            Ok(false)
        }
        status if status.is_server_error() => {
            warn!(
                status = status.as_u16(),
                bucket = %bucket,
                key = %key,
                object_url = %object_url,
                "s3 object HEAD returned server error"
            );
            Err((
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            ))
        }
        status => {
            warn!(
                status = status.as_u16(),
                bucket = %bucket,
                key = %key,
                object_url = %object_url,
                "s3 object HEAD returned unexpected status"
            );
            Err((
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            ))
        }
    }
}

fn sign_access_request(s3: &S3Cfg, bucket: &str, key: &str) -> DownloadSignResponse {
    let region = signing_region(s3);

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
    let host = s3_host(s3);
    let region = signing_region(s3);

    let signed = sign_request(
        "DELETE",
        &s3.access_key,
        &s3.secret_key,
        &host,
        &format!("/{}/{}", bucket, key),
        "",
        region,
    );

    DeleteSignResponse {
        method: "DELETE".to_string(),
        delete_url: s3_api_object_url(s3, bucket, key),
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
const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

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

fn validate_upload_body(body: &Bytes) -> Result<(), ApiError> {
    if body.is_empty() || body.len() > MAX_UPLOAD_BYTES {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(error_code::INVALID_PARAMS.into()),
        ));
    }
    Ok(())
}

fn validate_request_content_type(headers: &AxumHeaderMap, expected: &str) -> Result<(), ApiError> {
    let Some(content_type) = headers.get(header::CONTENT_TYPE) else {
        return Ok(());
    };
    let Ok(content_type) = content_type.to_str() else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(error_code::INVALID_PARAMS.into()),
        ));
    };
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let expected_media_type = expected
        .split(';')
        .next()
        .unwrap_or(expected)
        .trim()
        .to_ascii_lowercase();

    if media_type == expected_media_type || media_type == "application/octet-stream" {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(error_code::INVALID_PARAMS.into()),
        ))
    }
}

fn validate_image_payload(ext: &str, body: &[u8]) -> Result<(), ApiError> {
    let ok = match ext {
        "jpg" | "jpeg" => body.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => body.starts_with(b"\x89PNG\r\n\x1A\n"),
        "webp" => body.len() >= 12 && body.starts_with(b"RIFF") && &body[8 .. 12] == b"WEBP",
        "gif" => body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a"),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(error_code::INVALID_PARAMS.into()),
        ))
    }
}

fn validate_document_payload(ext: &str, body: &[u8]) -> Result<(), ApiError> {
    let ok = match ext {
        "pdf" => body.starts_with(b"%PDF-"),
        "doc" | "xls" => body.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]),
        "docx" | "xlsx" => body.starts_with(b"PK\x03\x04"),
        "txt" | "md" => std::str::from_utf8(body).is_ok(),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(error_code::INVALID_PARAMS.into()),
        ))
    }
}

fn sha256_hex(body: &[u8]) -> String {
    let digest = sha2::Sha256::digest(body);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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

fn normalize_object_key_with_bucket<'a>(input: &str, s3: &'a S3Cfg) -> Option<(&'a str, String)> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }

    if s3.provider == S3Provider::CloudflareR2 {
        if let Some(public_bucket_url) = normalized_public_bucket_url(s3) {
            if let Some(path) = strip_url_base(raw, public_bucket_url) {
                if !path.is_empty() {
                    return Some((&s3.public_bucket, path));
                }
            }
        }
        if let Some(private_bucket_url) = normalized_private_bucket_url(s3) {
            if let Some(path) = strip_url_base(raw, private_bucket_url) {
                if !path.is_empty() {
                    return Some((&s3.private_bucket, path));
                }
            }
        }
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

    if let Some(stripped) = path.strip_prefix(&format!("{}/", s3.private_bucket)) {
        return Some((&s3.private_bucket, stripped.to_string()));
    }
    if let Some(stripped) = path.strip_prefix(&format!("{}/", s3.public_bucket)) {
        return Some((&s3.public_bucket, stripped.to_string()));
    }

    if path.is_empty() {
        return None;
    }

    if path.starts_with("avatars/") {
        Some((&s3.public_bucket, path))
    } else {
        Some((&s3.private_bucket, std::mem::take(&mut path)))
    }
}

fn strip_url_base(input: &str, base_url: &str) -> Option<String> {
    let input = input.trim_end_matches('/');
    let base_url = base_url.trim_end_matches('/');
    let stripped = input.strip_prefix(base_url)?;
    Some(stripped.trim_start_matches('/').to_string())
}

fn user_scope_key(user_id: &str, s3: &S3Cfg) -> String {
    let uuid = Uuid::parse_str(user_id).unwrap_or_else(|_| {
        let salt = s3
            .user_key_salt
            .as_deref()
            .unwrap_or("change-me-user-key-salt");
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest as _;
        hasher.update(salt.as_bytes());
        hasher.update(b":");
        hasher.update(user_id.as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[.. 16]);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_s3_cfg(provider: S3Provider) -> S3Cfg {
        S3Cfg {
            provider,
            endpoint: "https://example-s3.local".to_string(),
            public_bucket: "public-bucket".to_string(),
            private_bucket: "private-bucket".to_string(),
            region: Some("us-east-1".to_string()),
            access_key: "access".to_string(),
            secret_key: "secret".to_string(),
            user_key_salt: Some("salt".to_string()),
            public_bucket_url: None,
            private_bucket_url: None,
        }
    }

    #[test]
    fn normalize_delete_key_preserves_s3_bucket_paths() {
        let s3 = test_s3_cfg(S3Provider::S3);

        let (bucket, key) = normalize_object_key_with_bucket(
            "https://example-s3.local/public-bucket/avatars/u1",
            &s3,
        )
        .expect("public key");

        assert_eq!(bucket, "public-bucket");
        assert_eq!(key, "avatars/u1");
    }

    #[test]
    fn normalize_delete_key_accepts_cloudflare_public_bucket_url() {
        let mut s3 = test_s3_cfg(S3Provider::CloudflareR2);
        s3.public_bucket_url = Some("https://assets.example.com/".to_string());

        let (bucket, key) =
            normalize_object_key_with_bucket("https://assets.example.com/avatars/u1", &s3)
                .expect("public key");

        assert_eq!(bucket, "public-bucket");
        assert_eq!(key, "avatars/u1");
    }

    #[test]
    fn normalize_delete_key_accepts_cloudflare_private_bucket_url() {
        let mut s3 = test_s3_cfg(S3Provider::CloudflareR2);
        s3.private_bucket_url = Some("https://vault.example.com/".to_string());

        let (bucket, key) = normalize_object_key_with_bucket(
            "https://vault.example.com/users/u1/images/file.png",
            &s3,
        )
        .expect("private key");

        assert_eq!(bucket, "private-bucket");
        assert_eq!(key, "users/u1/images/file.png");
    }

    #[test]
    fn cloudflare_upload_url_still_uses_s3_api_endpoint() {
        let mut s3 = test_s3_cfg(S3Provider::CloudflareR2);
        s3.public_bucket_url = Some("https://assets.example.com".to_string());

        assert_eq!(
            s3_api_object_url(&s3, &s3.public_bucket, "avatars/u1"),
            "https://example-s3.local/public-bucket/avatars/u1"
        );
    }

    #[test]
    fn sha256_hex_matches_known_digest() {
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn image_payload_validation_checks_magic_bytes() {
        assert!(validate_image_payload("png", b"\x89PNG\r\n\x1A\nrest").is_ok());
        assert!(validate_image_payload("png", b"not a png").is_err());
    }

    #[test]
    fn document_payload_validation_checks_magic_bytes() {
        assert!(validate_document_payload("pdf", b"%PDF-1.7\n").is_ok());
        assert!(validate_document_payload("docx", b"PK\x03\x04rest").is_ok());
        assert!(validate_document_payload("pdf", b"not a pdf").is_err());
    }

    #[test]
    fn object_url_prefers_configured_bucket_urls() {
        let mut s3 = test_s3_cfg(S3Provider::CloudflareR2);
        s3.public_bucket_url = Some("https://assets.example.com/".to_string());
        s3.private_bucket_url = Some("https://vault.example.com".to_string());

        assert_eq!(
            object_public_or_api_url(&s3, &s3.public_bucket, "avatars/u1"),
            "https://assets.example.com/avatars/u1"
        );
        assert_eq!(
            object_public_or_api_url(&s3, &s3.private_bucket, "users/u1/docs/a.pdf"),
            "https://vault.example.com/users/u1/docs/a.pdf"
        );
    }
}
