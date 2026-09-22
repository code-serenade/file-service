use std::sync::Arc;

use axum::{
    Extension, Json,
    body::Bytes,
    extract::Query,
    http::{HeaderMap as AxumHeaderMap, StatusCode, header},
};
use chrono::Utc;
use hmac::{Hmac, Mac};
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
    settings::{ReadPolicy, S3AddressingStyle, S3Cfg, S3Provider, StorageBucketCfg},
    utils::base62::encode_u128,
};

const AVATAR_BUCKET_ID: &str = "avatar";
const IMAGE_BUCKET_ID: &str = "image";
const DOCUMENT_BUCKET_ID: &str = "document";
const MEDIA_BUCKET_ID: &str = "media";

pub async fn upload_avatar(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
) -> ResponseResult<UploadSignResponse> {
    let bucket = storage_bucket(&s3, AVATAR_BUCKET_ID)?;
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = avatar_key(bucket, &user_scope);
    Ok(
        sign_upload_request(&s3, bucket, &key, None, Some("inline"), false)
            .into_common_response()
            .to_json(),
    )
}

pub async fn upload_image(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
) -> ResponseResult<UploadSignResponse> {
    let bucket = storage_bucket(&s3, IMAGE_BUCKET_ID)?;
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
    let key = image_key(bucket, &user_scope, &filename, &ext);
    let already_uploaded = object_exists(&s3, &bucket.bucket, &key).await?;
    Ok(sign_upload_request(
        &s3,
        bucket,
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
    let bucket = storage_bucket(&s3, DOCUMENT_BUCKET_ID)?;
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
    let key = document_key(bucket, &user_scope, &filename, &ext);
    let already_uploaded = object_exists(&s3, &bucket.bucket, &key).await?;
    Ok(sign_upload_request(
        &s3,
        bucket,
        &key,
        Some(content_type),
        Some("inline"),
        already_uploaded,
    )
    .into_common_response()
    .to_json())
}

pub async fn upload_media(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
) -> ResponseResult<UploadSignResponse> {
    let bucket = storage_bucket(&s3, MEDIA_BUCKET_ID)?;
    let ext = normalize_and_validate_ext(&query.ext, MEDIA_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let filename = normalize_and_validate_sha256_filename(query.filename.as_deref()).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    let content_type = content_type_for_media_ext(&ext);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = media_key(bucket, &user_scope, &filename, &ext);
    let already_uploaded = object_exists(&s3, &bucket.bucket, &key).await?;
    Ok(sign_upload_request(
        &s3,
        bucket,
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
    let bucket = storage_bucket(&s3, AVATAR_BUCKET_ID)?;
    let ext = normalize_and_validate_ext(&query.ext, IMAGE_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    validate_upload_body(&body)?;
    validate_image_payload(&ext, &body)?;

    let content_type = content_type_for_image_ext(&ext);
    validate_request_content_type(&headers, content_type)?;

    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = avatar_key(bucket, &user_scope);
    server_upload_object(&s3, bucket, &key, content_type, &body, false).await
}

pub async fn put_upload_image(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
    headers: AxumHeaderMap,
    body: Bytes,
) -> ResponseResult<ServerUploadResponse> {
    let bucket = storage_bucket(&s3, IMAGE_BUCKET_ID)?;
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
    let key = image_key(bucket, &user_scope, &hash, &ext);
    server_upload_object(&s3, bucket, &key, content_type, &body, true).await
}

pub async fn put_upload_document(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
    headers: AxumHeaderMap,
    body: Bytes,
) -> ResponseResult<ServerUploadResponse> {
    let bucket = storage_bucket(&s3, DOCUMENT_BUCKET_ID)?;
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
    let key = document_key(bucket, &user_scope, &hash, &ext);
    server_upload_object(&s3, bucket, &key, content_type, &body, true).await
}

pub async fn put_upload_media(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<UploadExtQuery>,
    headers: AxumHeaderMap,
    body: Bytes,
) -> ResponseResult<ServerUploadResponse> {
    let bucket = storage_bucket(&s3, MEDIA_BUCKET_ID)?;
    let ext = normalize_and_validate_ext(&query.ext, MEDIA_ALLOWED_EXTS).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;
    validate_upload_body(&body)?;
    validate_media_payload(&ext, &body)?;

    let content_type = content_type_for_media_ext(&ext);
    validate_request_content_type(&headers, content_type)?;

    let hash = sha256_hex(&body);
    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    let key = media_key(bucket, &user_scope, &hash, &ext);
    server_upload_object(&s3, bucket, &key, content_type, &body, true).await
}

pub async fn access_sign(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<AccessSignQuery>,
) -> ResponseResult<DownloadSignResponse> {
    let bucket_id = requested_bucket_id(&s3, query.bucket_id.as_deref(), &query.key)?;
    let bucket = storage_bucket(&s3, &bucket_id)?;
    let normalized_key = normalize_object_key(&query.key, &s3, bucket).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;

    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    if !is_user_owned_key(&bucket_id, bucket, &user_scope, &normalized_key) {
        return Err((StatusCode::FORBIDDEN, Json(error_code::FORBIDDEN.into())));
    }

    let response = if bucket.read_policy == ReadPolicy::Public {
        DownloadSignResponse {
            bucket_id: bucket_id.clone(),
            method: "GET".to_string(),
            download_url: s3_target(&s3, &s3.endpoint, &bucket.bucket, &normalized_key).url,
            key: normalized_key,
        }
    } else {
        sign_access_request(&s3, &bucket_id, &bucket.bucket, &normalized_key)
    };

    Ok(response.into_common_response().to_json())
}

pub async fn delete_sign(
    Extension(auth_user): Extension<AuthUser>,
    Extension(s3): Extension<Arc<S3Cfg>>,
    Query(query): Query<DeleteSignQuery>,
) -> ResponseResult<DeleteSignResponse> {
    let bucket_id = requested_bucket_id(&s3, query.bucket_id.as_deref(), &query.key)?;
    let bucket = storage_bucket(&s3, &bucket_id)?;
    let normalized_key = normalize_object_key(&query.key, &s3, bucket).ok_or((
        StatusCode::BAD_REQUEST,
        Json(error_code::INVALID_PARAMS.into()),
    ))?;

    let user_scope = user_scope_key(&auth_user.user_id, &s3);
    if !is_user_owned_key(&bucket_id, bucket, &user_scope, &normalized_key) {
        return Err((StatusCode::FORBIDDEN, Json(error_code::FORBIDDEN.into())));
    }

    Ok(
        sign_delete_request(&s3, &bucket_id, &bucket.bucket, &normalized_key)
            .into_common_response()
            .to_json(),
    )
}

async fn server_upload_object(
    s3: &S3Cfg,
    bucket: &StorageBucketCfg,
    key: &str,
    content_type: &str,
    body: &Bytes,
    check_existing: bool,
) -> ResponseResult<ServerUploadResponse> {
    let already_uploaded = if check_existing {
        object_exists(s3, &bucket.bucket, key).await?
    } else {
        false
    };

    if !already_uploaded {
        put_object_to_s3(s3, &bucket.bucket, key, content_type, body.clone()).await?;
    }

    Ok(ServerUploadResponse {
        bucket_id: bucket.id.clone(),
        key: key.to_string(),
        url: s3_target(s3, &s3.endpoint, &bucket.bucket, key).url,
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
    let endpoint = internal_s3_endpoint(s3);
    let target = s3_target(s3, endpoint, bucket, key);
    let region = signing_region(s3);
    let signed = sign_request(
        "PUT",
        &s3.access_key,
        &s3.secret_key,
        &target.host,
        &target.path,
        "",
        region,
    );

    let object_url = target.url;
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
    bucket: &StorageBucketCfg,
    key: &str,
    content_type: Option<&str>,
    content_disposition: Option<&str>,
    already_uploaded: bool,
) -> UploadSignResponse {
    let target = s3_target(s3, &s3.endpoint, &bucket.bucket, key);

    let region = signing_region(s3);

    let signed = sign_request(
        "PUT",
        &s3.access_key,
        &s3.secret_key,
        &target.host,
        &target.path,
        "",
        region,
    );

    UploadSignResponse {
        bucket_id: bucket.id.clone(),
        method: "PUT".to_string(),
        upload_url: target.url,
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

fn internal_s3_endpoint(s3: &S3Cfg) -> &str {
    s3.internal_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .unwrap_or(&s3.endpoint)
}

fn s3_host(endpoint: &str) -> String {
    endpoint
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string()
}

struct S3Target {
    url: String,
    host: String,
    path: String,
}

fn s3_target(s3: &S3Cfg, endpoint: &str, bucket: &str, key: &str) -> S3Target {
    let endpoint = endpoint.trim_end_matches('/');
    let key = key.trim_start_matches('/');
    if s3.addressing_style == S3AddressingStyle::VirtualHost {
        let (scheme, host) = endpoint.split_once("://").unwrap_or(("https", endpoint));
        let host = format!("{bucket}.{host}");
        return S3Target {
            url: format!("{scheme}://{host}/{key}"),
            host,
            path: format!("/{key}"),
        };
    }

    S3Target {
        url: format!("{endpoint}/{bucket}/{key}"),
        host: s3_host(endpoint),
        path: format!("/{bucket}/{key}"),
    }
}

fn storage_bucket<'a>(s3: &'a S3Cfg, bucket_id: &str) -> Result<&'a StorageBucketCfg, ApiError> {
    s3.buckets
        .iter()
        .find(|bucket| bucket.id == bucket_id)
        .ok_or_else(|| {
            (
                StatusCode::BAD_GATEWAY,
                Json(error_code::BAD_GATEWAY.into()),
            )
        })
}

fn requested_bucket_id(
    s3: &S3Cfg,
    requested_bucket_id: Option<&str>,
    key: &str,
) -> Result<String, ApiError> {
    if let Some(bucket_id) = requested_bucket_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        return Ok(bucket_id.to_string());
    }

    let mut matches = Vec::new();
    for bucket_id in [
        AVATAR_BUCKET_ID,
        IMAGE_BUCKET_ID,
        DOCUMENT_BUCKET_ID,
        MEDIA_BUCKET_ID,
    ] {
        let Ok(bucket) = storage_bucket(s3, bucket_id) else {
            continue;
        };
        let Some(normalized_key) = normalize_object_key(key, s3, bucket) else {
            continue;
        };
        if key_matches_bucket_id(bucket_id, bucket, &normalized_key) {
            matches.push(bucket_id);
        }
    }

    if matches.len() == 1 {
        Ok(matches[0].to_string())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(error_code::INVALID_PARAMS.into()),
        ))
    }
}

fn key_with_prefix(bucket: &StorageBucketCfg, suffix: &str) -> String {
    let prefix = bucket.key_prefix.trim_matches('/');
    let suffix = suffix.trim_start_matches('/');
    if prefix.is_empty() {
        suffix.to_string()
    } else {
        format!("{prefix}/{suffix}")
    }
}

fn avatar_key(bucket: &StorageBucketCfg, user_scope: &str) -> String {
    key_with_prefix(bucket, user_scope)
}

fn image_key(bucket: &StorageBucketCfg, user_scope: &str, hash: &str, ext: &str) -> String {
    key_with_prefix(bucket, &format!("{user_scope}/images/{hash}.{ext}"))
}

fn document_key(bucket: &StorageBucketCfg, user_scope: &str, hash: &str, ext: &str) -> String {
    key_with_prefix(bucket, &format!("{user_scope}/docs/{hash}.{ext}"))
}

fn media_key(bucket: &StorageBucketCfg, user_scope: &str, hash: &str, ext: &str) -> String {
    key_with_prefix(bucket, &format!("{user_scope}/media/{hash}.{ext}"))
}

fn key_matches_bucket_id(bucket_id: &str, bucket: &StorageBucketCfg, key: &str) -> bool {
    let prefix = bucket.key_prefix.trim_matches('/');
    let suffix = key
        .strip_prefix(prefix)
        .unwrap_or(key)
        .trim_start_matches('/');
    match bucket_id {
        AVATAR_BUCKET_ID => !suffix.is_empty() && !suffix.contains('/'),
        IMAGE_BUCKET_ID => suffix.contains("/images/"),
        DOCUMENT_BUCKET_ID => suffix.contains("/docs/"),
        MEDIA_BUCKET_ID => suffix.contains("/media/"),
        _ => false,
    }
}

fn signing_region(s3: &S3Cfg) -> Option<&str> {
    s3.region
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| (s3.provider == S3Provider::CloudflareR2).then_some("auto"))
}

async fn object_exists(s3: &S3Cfg, bucket: &str, key: &str) -> Result<bool, ApiError> {
    let endpoint = internal_s3_endpoint(s3);
    let target = s3_target(s3, endpoint, bucket, key);
    let region = signing_region(s3);

    let signed = sign_request(
        "HEAD",
        &s3.access_key,
        &s3.secret_key,
        &target.host,
        &target.path,
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

    let object_url = target.url;
    debug!(
        endpoint = %endpoint,
        host = %target.host,
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

fn sign_access_request(
    s3: &S3Cfg,
    bucket_id: &str,
    bucket: &str,
    key: &str,
) -> DownloadSignResponse {
    let region = signing_region(s3);

    let download_url = if s3.addressing_style == S3AddressingStyle::VirtualHost {
        presign_virtual_get(s3, bucket, key, region.unwrap_or("us-east-1"), 600)
    } else {
        presign_get_object(
            &s3.access_key,
            &s3.secret_key,
            bucket,
            key,
            region,
            &s3.endpoint,
            Some(600),
        )
    };

    DownloadSignResponse {
        bucket_id: bucket_id.to_string(),
        method: "GET".to_string(),
        download_url,
        key: key.to_string(),
    }
}

fn presign_virtual_get(
    s3: &S3Cfg,
    bucket: &str,
    key: &str,
    region: &str,
    expires_secs: u64,
) -> String {
    let target = s3_target(s3, &s3.endpoint, bucket, key);
    let now = Utc::now();
    let date_stamp = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let credential = format!(
        "{}/{}/{}/s3/aws4_request",
        s3.access_key, date_stamp, region
    );
    let query = format!(
        "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential={}&X-Amz-Date={}&X-Amz-Expires={}&\
         X-Amz-SignedHeaders=host",
        percent_encode(&credential),
        amz_date,
        expires_secs,
    );
    let canonical_request = format!(
        "GET\n{}\n{}\nhost:{}\n\nhost\nUNSIGNED-PAYLOAD",
        target.path, query, target.host
    );
    let scope = format!("{date_stamp}/{region}/s3/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex_10(canonical_request.as_bytes())
    );
    let date_key = hmac_sha256(format!("AWS4{}", s3.secret_key).as_bytes(), &date_stamp);
    let region_key = hmac_sha256(&date_key, region);
    let service_key = hmac_sha256(&region_key, "s3");
    let signing_key = hmac_sha256(&service_key, "aws4_request");
    let signature = hex::encode(hmac_sha256(&signing_key, &string_to_sign));
    format!("{}?{}&X-Amz-Signature={signature}", target.url, query)
}

fn hmac_sha256(key: &[u8], value: &str) -> Vec<u8> {
    let mut mac = Hmac::<sha2_10::Sha256>::new_from_slice(key).expect("valid HMAC key");
    mac.update(value.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn sha256_hex_10(value: &[u8]) -> String {
    use sha2_10::Digest as _;
    hex::encode(sha2_10::Sha256::digest(value))
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A' ..= b'Z' | b'a' ..= b'z' | b'0' ..= b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn sign_delete_request(s3: &S3Cfg, bucket_id: &str, bucket: &str, key: &str) -> DeleteSignResponse {
    let target = s3_target(s3, &s3.endpoint, bucket, key);
    let region = signing_region(s3);

    let signed = sign_request(
        "DELETE",
        &s3.access_key,
        &s3.secret_key,
        &target.host,
        &target.path,
        "",
        region,
    );

    DeleteSignResponse {
        bucket_id: bucket_id.to_string(),
        method: "DELETE".to_string(),
        delete_url: target.url,
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

fn is_user_owned_key(
    bucket_id: &str,
    bucket: &StorageBucketCfg,
    user_scope: &str,
    key: &str,
) -> bool {
    match bucket_id {
        AVATAR_BUCKET_ID => key == avatar_key(bucket, user_scope),
        IMAGE_BUCKET_ID => {
            key.starts_with(&key_with_prefix(bucket, &format!("{user_scope}/images/")))
        }
        DOCUMENT_BUCKET_ID => {
            key.starts_with(&key_with_prefix(bucket, &format!("{user_scope}/docs/")))
        }
        MEDIA_BUCKET_ID => {
            key.starts_with(&key_with_prefix(bucket, &format!("{user_scope}/media/")))
        }
        _ => false,
    }
}

const IMAGE_ALLOWED_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];
const DOCUMENT_ALLOWED_EXTS: &[&str] = &["pdf", "doc", "docx", "xls", "xlsx", "txt", "md"];
const MEDIA_ALLOWED_EXTS: &[&str] = &["mp4", "mov", "m4v", "wav", "mp3"];
pub(crate) const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

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

fn validate_media_payload(ext: &str, body: &[u8]) -> Result<(), ApiError> {
    let ok = match ext {
        "mp4" | "mov" | "m4v" => body.len() >= 12 && &body[4 .. 8] == b"ftyp",
        "wav" => body.len() >= 44 && body.starts_with(b"RIFF") && &body[8 .. 12] == b"WAVE",
        // Header screening only; consumers must fully decode before processing.
        "mp3" => {
            (body.len() >= 10 && body.starts_with(b"ID3") && (2 ..= 4).contains(&body[3]))
                || (body.len() >= 4
                    && body[0] == 0xff
                    && body[1] & 0xe0 == 0xe0
                    && body[1] & 0x18 != 0x08
                    && body[1] & 0x06 == 0x02
                    && body[2] >> 4 != 0
                    && body[2] >> 4 != 0x0f
                    && body[2] & 0x0c != 0x0c)
        }
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

fn normalize_object_key(input: &str, s3: &S3Cfg, bucket: &StorageBucketCfg) -> Option<String> {
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

    if s3.addressing_style == S3AddressingStyle::Path
        && let Some(stripped) = key.strip_prefix(&format!("{}/", bucket.bucket))
    {
        key = stripped.to_string();
    }

    if key.is_empty() { None } else { Some(key) }
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

fn content_type_for_media_ext(ext: &str) -> &'static str {
    match ext {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
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
            addressing_style: S3AddressingStyle::Path,
            internal_endpoint: None,
            buckets: vec![
                StorageBucketCfg {
                    id: AVATAR_BUCKET_ID.to_string(),
                    bucket: "avatar-bucket".to_string(),
                    read_policy: ReadPolicy::Public,
                    key_prefix: "avatars".to_string(),
                },
                StorageBucketCfg {
                    id: IMAGE_BUCKET_ID.to_string(),
                    bucket: "image-bucket".to_string(),
                    read_policy: ReadPolicy::Private,
                    key_prefix: "users".to_string(),
                },
                StorageBucketCfg {
                    id: DOCUMENT_BUCKET_ID.to_string(),
                    bucket: "document-bucket".to_string(),
                    read_policy: ReadPolicy::Private,
                    key_prefix: "users".to_string(),
                },
                StorageBucketCfg {
                    id: MEDIA_BUCKET_ID.to_string(),
                    bucket: "media-bucket".to_string(),
                    read_policy: ReadPolicy::Private,
                    key_prefix: "users".to_string(),
                },
            ],
            region: Some("us-east-1".to_string()),
            access_key: "access".to_string(),
            secret_key: "secret".to_string(),
            user_key_salt: Some("salt".to_string()),
        }
    }

    #[test]
    fn normalize_key_removes_path_style_bucket_prefix() {
        let s3 = test_s3_cfg(S3Provider::S3);
        let bucket = storage_bucket(&s3, AVATAR_BUCKET_ID).unwrap();
        let key = normalize_object_key(
            "https://example-s3.local/avatar-bucket/avatars/u1",
            &s3,
            bucket,
        )
        .expect("avatar key");
        assert_eq!(key, "avatars/u1");
    }

    #[test]
    fn bucket_config_owns_physical_bucket_selection() {
        let s3 = test_s3_cfg(S3Provider::CloudflareR2);
        let bucket = storage_bucket(&s3, AVATAR_BUCKET_ID).unwrap();

        assert_eq!(
            s3_target(&s3, &s3.endpoint, &bucket.bucket, "avatars/u1").url,
            "https://example-s3.local/avatar-bucket/avatars/u1"
        );
    }

    #[test]
    fn internal_s3_endpoint_only_applies_to_service_requests() {
        let mut s3 = test_s3_cfg(S3Provider::S3);
        s3.internal_endpoint = Some("http://127.0.0.1:29999/".to_string());

        assert_eq!(internal_s3_endpoint(&s3), "http://127.0.0.1:29999/");
        assert_eq!(s3_host(internal_s3_endpoint(&s3)), "127.0.0.1:29999");
        assert_eq!(
            s3_target(&s3, internal_s3_endpoint(&s3), "image-bucket", "image.png").url,
            "http://127.0.0.1:29999/image-bucket/image.png"
        );
        assert_eq!(
            s3_target(&s3, &s3.endpoint, "image-bucket", "image.png").url,
            "https://example-s3.local/image-bucket/image.png"
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
    fn media_payload_validation_checks_iso_base_media_signature() {
        assert!(validate_media_payload("mp4", b"\x00\x00\x00\x18ftypisomrest").is_ok());
        assert!(validate_media_payload("mov", b"\x00\x00\x00\x14ftypqt  rest").is_ok());
        assert!(validate_media_payload("mp4", b"not a video").is_err());
    }

    #[test]
    fn media_audio_headers_and_mime_types() {
        let mut wav = vec![0u8; 44];
        wav[.. 4].copy_from_slice(b"RIFF");
        wav[8 .. 12].copy_from_slice(b"WAVE");
        assert!(validate_media_payload("wav", &wav).is_ok());
        assert!(validate_media_payload("wav", b"RIFFshort").is_err());
        assert!(validate_media_payload("mp3", b"ID3\x04\x00\x00\x00\x00\x00\x00").is_ok());
        assert!(validate_media_payload("mp3", &[0xff, 0xfb, 0x90, 0]).is_ok());
        assert!(validate_media_payload("mp3", b"not audio").is_err());
        assert_eq!(content_type_for_media_ext("wav"), "audio/wav");
        assert_eq!(content_type_for_media_ext("mp3"), "audio/mpeg");
    }

    #[test]
    fn ownership_uses_logical_bucket_category_and_prefix() {
        let s3 = test_s3_cfg(S3Provider::CloudflareR2);
        let avatar = storage_bucket(&s3, AVATAR_BUCKET_ID).unwrap();
        let image = storage_bucket(&s3, IMAGE_BUCKET_ID).unwrap();
        assert_eq!(
            is_user_owned_key(AVATAR_BUCKET_ID, avatar, "u1", "avatars/u1"),
            true
        );
        assert_eq!(
            is_user_owned_key(IMAGE_BUCKET_ID, image, "u1", "users/u1/images/a.png"),
            true
        );
    }

    #[test]
    fn legacy_key_without_bucket_id_resolves_its_logical_category() {
        let s3 = test_s3_cfg(S3Provider::S3);

        assert_eq!(
            requested_bucket_id(&s3, None, "avatars/u1").unwrap(),
            AVATAR_BUCKET_ID
        );
        assert_eq!(
            requested_bucket_id(&s3, None, "users/u1/images/a.png").unwrap(),
            IMAGE_BUCKET_ID
        );
        assert_eq!(
            requested_bucket_id(&s3, Some(DOCUMENT_BUCKET_ID), "anything").unwrap(),
            DOCUMENT_BUCKET_ID
        );
        assert_eq!(
            requested_bucket_id(&s3, None, "users/u1/media/a.mp4").unwrap(),
            MEDIA_BUCKET_ID
        );
    }
}
