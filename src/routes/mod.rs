mod s3;

use std::sync::Arc;

use axum::{Extension, Router, middleware::from_fn};
use toolcraft_axum_kit::middleware::{auth_mw::auth, cors::create_cors};
use toolcraft_jwt::VerifyJwt;

use crate::settings::S3Cfg;

pub fn create_routes(jwt: Arc<VerifyJwt>, s3: Arc<S3Cfg>) -> Router {
    let cors = create_cors();

    let protected_router = Router::new()
        .nest("/sign", s3::protected_routes().layer(Extension(s3.clone())))
        .route_layer(from_fn(auth::<VerifyJwt>));

    let access_router = Router::new().nest("/sign", s3::access_routes().layer(Extension(s3)));

    Router::new()
        .nest("/file", protected_router)
        .nest("/file", access_router)
        .layer(Extension(jwt))
        .layer(cors)
}
