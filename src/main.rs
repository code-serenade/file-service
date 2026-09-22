mod error;
mod handlers;
mod logging;
mod models;
mod routes;
mod settings;
pub mod utils;

use std::{sync::Arc, time::Duration};

use settings::Settings;
use toolcraft_axum_kit::http_server;
use tracing::{info, warn};

use crate::logging::init_tracing_to_file;

#[tokio::main]
async fn main() {
    init_tracing_to_file();
    let mut settings = Settings::load("config/services.toml").unwrap();
    settings.validate_storage_buckets().unwrap();

    let jwt = Arc::new(settings.jwt_verify.fetch_verify_jwt().await.unwrap());
    apply_shared_user_key_salt(&mut settings);
    let s3 = Arc::new(settings.s3);
    let avatar_provider_url = s3.avatar_public_base_url().unwrap();
    let auth_cfg = settings.jwt_verify;
    tokio::spawn(register_avatar_provider_forever(
        auth_cfg,
        avatar_provider_url,
    ));
    let r2 = settings.r2.map(Arc::new);
    let router = routes::create_routes(jwt, s3, r2);
    let http_task = http_server::start(settings.http.port, router);

    let _ = tokio::join!(http_task);
}

async fn register_avatar_provider_forever(
    auth_cfg: settings::JwtVerifyRemoteCfg,
    avatar_provider_url: String,
) {
    loop {
        match auth_cfg
            .register_avatar_provider(&avatar_provider_url)
            .await
        {
            Ok(()) => info!(%avatar_provider_url, "registered avatar provider with auth"),
            Err(error) => warn!(%error, "failed to register avatar provider with auth; retrying"),
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

fn apply_shared_user_key_salt(settings: &mut Settings) {
    let Some(shared_salt) = settings.storage.user_key_salt.clone() else {
        return;
    };

    if settings.s3.user_key_salt.is_none() {
        settings.s3.user_key_salt = Some(shared_salt.clone());
    }

    if let Some(r2) = &mut settings.r2
        && r2.user_key_salt.is_none()
    {
        r2.user_key_salt = Some(shared_salt);
    }
}
