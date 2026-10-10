mod error;
mod handlers;
mod logging;
mod models;
mod routes;
mod settings;
pub mod utils;

use std::{process::ExitCode, sync::Arc};

use settings::Settings;
use toolcraft_axum_kit::http_server;
use tracing::error;

use crate::logging::init_tracing_to_file;

#[tokio::main]
async fn main() -> ExitCode {
    init_tracing_to_file();
    let mut settings = Settings::load("config/services.toml").unwrap();
    settings.validate_storage_buckets().unwrap();

    let jwt = Arc::new(settings.jwt_verify.fetch_verify_jwt().await.unwrap());
    apply_shared_user_key_salt(&mut settings);
    let s3 = Arc::new(settings.s3);
    let router = routes::create_routes(jwt, s3);
    let port = settings.http.port;

    match http_server::start(port, router).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(port, error = %err, "HTTP server failed");
            eprintln!("HTTP server failed on 0.0.0.0:{port}: {err}");
            ExitCode::FAILURE
        }
    }
}

fn apply_shared_user_key_salt(settings: &mut Settings) {
    let Some(shared_salt) = settings.storage.user_key_salt.clone() else {
        return;
    };

    if settings.s3.user_key_salt.is_none() {
        settings.s3.user_key_salt = Some(shared_salt.clone());
    }
}
