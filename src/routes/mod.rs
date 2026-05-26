mod s3;

use std::sync::Arc;

use axum::{Extension, Router, middleware::from_fn};
use toolcraft_axum_kit::middleware::{auth_mw::auth, cors::create_cors};
use toolcraft_jwt::VerifyJwt;

use crate::settings::S3Cfg;

pub fn create_routes(jwt: Arc<VerifyJwt>, s3: Arc<S3Cfg>, r2: Option<Arc<S3Cfg>>) -> Router {
    let cors = create_cors();

    let api_router = Router::new()
        .nest("/sign", s3::s3_routes().layer(Extension(s3)))
        .merge(r2_routes(r2))
        .route_layer(from_fn(auth::<VerifyJwt>));

    Router::new()
        .nest("/file", api_router)
        .layer(Extension(jwt))
        .layer(cors)
}

fn r2_routes(r2: Option<Arc<S3Cfg>>) -> Router {
    if let Some(r2) = r2 {
        Router::new().nest("/r2/sign", s3::s3_routes().layer(Extension(r2)))
    } else {
        Router::new()
    }
}
