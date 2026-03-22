use axum::Router;
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

/// Returns a Router serving the embedded dashboard SPA at `/ui`.
///
/// Uses `FallbackBehavior::Ok` so all unrecognized paths under `/ui/` return
/// `index.html` with 200 -- the SPA's hash router handles client-side routing (D-14).
pub fn router() -> Router {
    let serve = ServeEmbed::<DashboardAssets>::with_parameters(
        Some("index.html".to_owned()),
        FallbackBehavior::Ok,
        Some("index.html".to_owned()),
    );
    Router::new().nest_service("/ui", serve)
}
