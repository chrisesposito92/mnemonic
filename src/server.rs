use axum::{routing::get, Json, Router};
use serde_json::Value;
use tracing_subscriber::prelude::*;

/// Initializes the tracing subscriber with pretty-printed output and EnvFilter.
///
/// Defaults to `info` level for the `mnemonic` crate; respects RUST_LOG env var for overrides.
pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mnemonic=info".parse().unwrap()),
        )
        .init();
}

/// Shared application state passed to all axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: std::sync::Arc<tokio_rusqlite::Connection>,
    pub config: std::sync::Arc<crate::config::Config>,
}

/// Constructs the axum Router with all routes wired to AppState.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(state)
}

/// GET /health — returns {"status":"ok"} with HTTP 200.
async fn health_handler() -> Json<Value> {
    Json(serde_json::json!({"status": "ok"}))
}

/// Binds a TCP listener and serves the axum application.
pub async fn serve(
    config: &crate::config::Config,
    state: AppState,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(address = %addr, "server listening");
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
