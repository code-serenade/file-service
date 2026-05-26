mod error;
mod handlers;
mod logging;
mod models;
mod routes;
mod settings;
pub mod utils;

use std::sync::Arc;

use settings::Settings;
use toolcraft_axum_kit::http_server;

use crate::logging::init_tracing_to_file;

#[tokio::main]
async fn main() {
    init_tracing_to_file();
    let mut settings = Settings::load("config/services.toml").unwrap();

    let jwt = Arc::new(settings.jwt_verify.fetch_verify_jwt().await.unwrap());
    apply_shared_user_key_salt(&mut settings);
    let s3 = Arc::new(settings.s3);
    let r2 = settings.r2.map(Arc::new);
    let router = routes::create_routes(jwt, s3, r2);
    let http_task = http_server::start(settings.http.port, router);

    let _ = tokio::join!(http_task);
}

fn apply_shared_user_key_salt(settings: &mut Settings) {
    let Some(shared_salt) = settings.storage.user_key_salt.clone() else {
        return;
    };

    if settings.s3.user_key_salt.is_none() {
        settings.s3.user_key_salt = Some(shared_salt.clone());
    }

    if let Some(r2) = &mut settings.r2 {
        if r2.user_key_salt.is_none() {
            r2.user_key_salt = Some(shared_salt);
        }
    }
}
