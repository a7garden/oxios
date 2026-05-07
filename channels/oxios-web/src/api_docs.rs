//! OpenAPI specification and Swagger UI setup.
//!
//! Provides a minimal OpenAPI 3.0 spec that can be populated
//! incrementally as routes are documented with utoipa annotations.

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityAddon};
use utoipa::OpenApi;

/// Build the OpenAPI specification for the Oxios web API.
pub fn build_openapi() -> utoipa::openapi::OpenApi {
    utoipa::openapi::OpenApiBuilder::new()
        .info(
            utoipa::openapi::InfoBuilder::new()
                .title("Oxios API")
                .version(env!("CARGO_PKG_VERSION"))
                .description(Some("Oxios agent orchestration platform API"))
                .build(),
        )
        .paths(utoipa::openapi::PathsBuilder::new().build())
        .components(Some(
            utoipa::openapi::ComponentsBuilder::new()
                .security_scheme(
                    "bearer_auth",
                    utoipa::openapi::SecurityScheme::Http(
                        HttpBuilder::new()
                            .scheme(HttpAuthScheme::Bearer)
                            .bearer_format("JWT")
                            .build(),
                    ),
                )
                .build(),
        )
        .build()
}
