use axum::Router;
use axum::middleware::map_response;
use axum::response::Response;
use axum_embed::{FallbackBehavior, ServeEmbed};
use rust_embed::RustEmbed;

/// Embedded dashboard SPA assets.
///
/// In release builds, files from `dashboard/dist/` are embedded at compile time.
/// If `dashboard/dist/` does not exist, the build fails with a compile-time error
/// (this is intentional -- do NOT add `#[allow_missing = true]`).
#[derive(RustEmbed, Clone)]
#[folder = "dashboard/dist/"]
struct DashboardAssets;

/// Injects Content-Security-Policy header on all /ui/ responses (AUTH-02).
///
/// Policy: vite-plugin-singlefile inlines all JS/CSS into index.html --
/// 'unsafe-inline' is required. No external connections from the dashboard
/// (all API calls go to 'self'). Safe for self-hosted developer tool.
const CONTENT_SECURITY_POLICY: &str =
    "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'";

async fn add_csp(response: Response) -> Response {
    let (mut parts, body) = response.into_parts();
    parts.headers.insert(
        axum::http::header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    Response::from_parts(parts, body)
}

/// Returns a Router serving the embedded dashboard SPA at `/ui`.
///
/// Uses `FallbackBehavior::Ok` so all unrecognized paths under `/ui/` return
/// `index.html` with 200 -- the SPA's hash router handles client-side routing (D-14).
/// Wraps responses with a CSP header layer (AUTH-02).
pub fn router() -> Router {
    let serve = ServeEmbed::<DashboardAssets>::with_parameters(
        Some("index.html".to_owned()),
        FallbackBehavior::Ok,
        Some("index.html".to_owned()),
    );
    Router::new()
        .nest_service("/ui", serve)
        .layer(map_response(add_csp))
}
