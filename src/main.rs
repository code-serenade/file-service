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
    let settings = Settings::load("config/services.toml").unwrap();

    let jwt = Arc::new(settings.jwt_verify.fetch_verify_jwt().await.unwrap());
    let s3 = Arc::new(settings.s3);
    let router = routes::create_routes(jwt, s3);
    let http_task = http_server::start(settings.http.port, router);

    let _ = tokio::join!(http_task);
}
