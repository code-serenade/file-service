mod s3;

use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::Request,
    http::{StatusCode, header},
    middleware::{Next, from_fn},
    response::Response,
};
use toolcraft_axum_kit::middleware::{auth_mw::auth, cors::create_cors};
use toolcraft_jwt::VerifyJwt;

use crate::settings::S3Cfg;

pub fn create_routes(jwt: Arc<VerifyJwt>, s3: Arc<S3Cfg>) -> Router {
    let cors = create_cors();

    let protected_router = Router::new()
        .nest("/sign", s3::protected_routes().layer(Extension(s3.clone())))
        .route_layer(from_fn(auth::<VerifyJwt>));

    let access_router = Router::new()
        .nest("/sign", s3::access_routes().layer(Extension(s3)))
        .route_layer(from_fn(optional_auth));

    Router::new()
        .nest("/file", protected_router)
        .nest("/file", access_router)
        .layer(Extension(jwt))
        .layer(cors)
}

async fn optional_auth(req: Request, next: Next) -> Result<Response, StatusCode> {
    if req.headers().contains_key(header::AUTHORIZATION) {
        auth::<VerifyJwt>(req, next).await
    } else {
        Ok(next.run(req).await)
    }
}
