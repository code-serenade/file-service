mod s3;

use std::sync::Arc;

use axum::{Extension, Router, middleware::from_fn};
use toolcraft_axum_kit::middleware::{auth_mw::auth, cors::create_cors};
use toolcraft_jwt::VerifyJwt;
use utoipa::{
    OpenApi,
    openapi::security::{ApiKey, SecurityScheme},
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{routes::s3::S3Api, settings::S3Cfg};

#[derive(OpenApi)]
#[openapi(
        nest(
            (path = "/sign", api = S3Api),
        ),
    )]
struct ApiDoc;

pub fn create_routes(jwt: Arc<VerifyJwt>, s3: Arc<S3Cfg>) -> Router {
    let cors = create_cors();
    let mut doc = ApiDoc::openapi();
    doc.components
        .get_or_insert_with(Default::default)
        .add_security_scheme(
            "Bearer",
            SecurityScheme::ApiKey(ApiKey::Header(
                utoipa::openapi::security::ApiKeyValue::with_description(
                    "Authorization",
                    "请输入格式：Bearer <token>",
                ),
            )),
        );

    Router::new()
        .nest("/sign", s3::s3_routes())
        .route_layer(from_fn(auth::<VerifyJwt>))
        .layer(Extension(jwt))
        .layer(Extension(s3))
        .layer(cors)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", doc))
}
