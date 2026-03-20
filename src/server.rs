use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::Value;
use tracing_subscriber::prelude::*;
use crate::compaction::CompactRequest;
use crate::error::ApiError;
use crate::service::{CreateMemoryRequest, ListParams, SearchParams};

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
    pub service: std::sync::Arc<crate::service::MemoryService>,
    pub compaction: std::sync::Arc<crate::compaction::CompactionService>,
    #[allow(dead_code)] // No route middleware until Phase 12
    pub key_service: std::sync::Arc<crate::auth::KeyService>,
}

/// Constructs the axum Router with all routes wired to AppState.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/memories", post(create_memory_handler).get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/{id}", delete(delete_memory_handler))
        .route("/memories/compact", post(compact_memories_handler))
        .with_state(state)
}

/// GET /health — returns {"status":"ok"} with HTTP 200.
async fn health_handler() -> Json<Value> {
    Json(serde_json::json!({"status": "ok"}))
}

/// POST /memories — creates a new memory and returns 201 Created.
async fn create_memory_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let memory = state.service.create_memory(body).await?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::to_value(memory).unwrap())))
}

/// GET /memories/search — semantic search returning ranked results with distance.
async fn search_memories_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = state.service.search_memories(params).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}

/// GET /memories — paginated list of memories with optional filters.
async fn list_memories_handler(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = state.service.list_memories(params).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}

/// DELETE /memories/:id — deletes a memory by ID and returns the deleted object.
async fn delete_memory_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let memory = state.service.delete_memory(id).await?;
    Ok(Json(serde_json::to_value(memory).unwrap()))
}

/// POST /memories/compact — triggers memory compaction for an agent.
async fn compact_memories_handler(
    State(state): State<AppState>,
    Json(body): Json<CompactRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.agent_id.trim().is_empty() {
        return Err(ApiError::BadRequest("agent_id must not be empty".to_string()));
    }
    if let Some(t) = body.threshold {
        if !(0.0..=1.0).contains(&t) {
            return Err(ApiError::BadRequest("threshold must be between 0.0 and 1.0".to_string()));
        }
    }
    if let Some(m) = body.max_candidates {
        if m == 0 {
            return Err(ApiError::BadRequest("max_candidates must be greater than 0".to_string()));
        }
    }
    let response = state.compaction.compact(body).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
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
