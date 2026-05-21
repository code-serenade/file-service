mod s3;

use std::sync::Arc;

use axum::{Extension, Router, middleware::from_fn};
use toolcraft_axum_kit::middleware::{auth_mw::auth, cors::create_cors};
use toolcraft_jwt::VerifyJwt;

use crate::settings::S3Cfg;

pub fn create_routes(jwt: Arc<VerifyJwt>, s3: Arc<S3Cfg>) -> Router {
    let cors = create_cors();

    let api_router = Router::new()
        .nest("/sign", s3::s3_routes())
        .route_layer(from_fn(auth::<VerifyJwt>));

    Router::new()
        .nest("/file", api_router)
        .layer(Extension(jwt))
        .layer(Extension(s3))
        .layer(cors)
}
