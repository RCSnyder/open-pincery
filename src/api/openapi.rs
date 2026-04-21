//! AC-44 (v8): OpenAPI 3.1 spec aggregator.
//!
//! `ApiDoc` is the single source of truth for the public HTTP contract.
//! Every annotated handler and every `ToSchema`-deriving DTO across
//! `src/api/*` is registered here. The `/openapi.json` and
//! `/openapi.yaml` endpoints serve it unauthenticated, sharing the
//! `/health` rate-limit bucket (i.e. none of the per-IP throttles in
//! front of either the authed or unauthed API surfaces).
//!
//! Slice 1a registers only `/api/me`. Slice 1b extends the `paths(...)`
//! list to cover every route in `api::router()`; AC-52a enforces that
//! coverage with a grep-style lint.

use axum::{extract::State, response::IntoResponse, routing::get, Router};
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use super::AppState;

/// Adds the `bearerAuth` HTTP security scheme to the generated spec.
pub struct BearerAuthAddon;

impl Modify for BearerAuthAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("session-token")
                    .description(Some(
                        "Session token returned by POST /api/bootstrap or POST /api/login.",
                    ))
                    .build(),
            ),
        );
    }
}

/// Aggregated OpenAPI document. Handlers are added one per v8 BUILD
/// slice; slice 1a registers only `/api/me`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Open Pincery API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Machine-readable contract for the Open Pincery HTTP surface. \
                       Every endpoint that appears in `api::router()` is listed here; \
                       the same schema drives `pcy` CLI generation and the MCP tool bridge.",
        license(name = "MIT OR Apache-2.0"),
    ),
    paths(
        super::me::me_handler,
    ),
    components(schemas(super::me::MeResponse)),
    modifiers(&BearerAuthAddon),
    tags(
        (name = "me", description = "Session introspection"),
    ),
)]
pub struct ApiDoc;

/// Serialise the document to a `serde_json::Value` and force the top-
/// level `openapi` field to `"3.1.0"`. utoipa 5.x emits `3.1.0` by
/// default already, but AC-44 is explicit about the string so we
/// normalise defensively — a future utoipa default-version change
/// will not silently break the contract.
fn spec_value() -> serde_json::Value {
    let doc = ApiDoc::openapi();
    let mut v = serde_json::to_value(&doc).expect("OpenAPI doc must be serialisable");
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "openapi".to_string(),
            serde_json::Value::String("3.1.0".to_string()),
        );
    }
    v
}

async fn openapi_json(State(_state): State<AppState>) -> impl IntoResponse {
    axum::Json(spec_value())
}

async fn openapi_yaml(State(_state): State<AppState>) -> impl IntoResponse {
    // utoipa 5.x `yaml` feature gives `OpenApi::to_yaml()`. We serialise
    // the unmodified document for YAML (it already emits `openapi: "3.1.0"`);
    // a failure here is a config error, not a request-time error.
    let yaml = ApiDoc::openapi()
        .to_yaml()
        .unwrap_or_else(|e| format!("# error serializing OpenAPI to YAML: {e}\n"));
    (
        [(axum::http::header::CONTENT_TYPE, "application/yaml")],
        yaml,
    )
}

/// Router for the two openapi endpoints. Mounted on the outermost
/// router alongside `/health` and `/ready` so it bypasses both auth
/// middleware and per-IP rate limiting — the spec is public contract.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/openapi.json", get(openapi_json))
        .route("/openapi.yaml", get(openapi_yaml))
}
