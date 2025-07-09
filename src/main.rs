mod error;
mod handlers;
mod logging;
mod models;
mod routes;
mod settings;

use std::sync::Arc;

use service_utils_rs::services::{http::http_server, jwt::Jwt};
use settings::Settings;

use crate::logging::init_tracing_to_file;

#[tokio::main]
async fn main() {
    init_tracing_to_file();
    let settings = Settings::load("config/services.toml").unwrap();

    let jwt = Arc::new(Jwt::new(settings.jwt));
    let s3 = Arc::new(settings.s3);
    let router = routes::create_routes(jwt, s3);
    let http_task = http_server::start(settings.http.port, router);

    let _ = tokio::join!(http_task);
}
