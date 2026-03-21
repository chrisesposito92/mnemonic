use axum::{
    extract::{Extension, Path, Query, State},
    middleware,
    routing::{delete, get, post},
    Json, Router,
};

#[derive(serde::Deserialize)]
struct CreateKeyRequest {
    name: String,
    agent_id: Option<String>,
}
use crate::auth::{auth_middleware, AuthContext};
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
    pub key_service: std::sync::Arc<crate::auth::KeyService>,
}

/// Constructs the axum Router with all routes wired to AppState.
///
/// Protected routes (`/memories*`) are wrapped with `route_layer` so the auth
/// middleware only applies to matched routes, not unmatched ones (per D-01, D-02).
/// Public routes (`/health`) have no auth middleware applied.
pub fn build_router(state: AppState) -> Router {
    // Protected routes: auth middleware applies via route_layer (per D-01, D-02)
    let protected = Router::new()
        .route("/memories", post(create_memory_handler).get(list_memories_handler))
        .route("/memories/search", get(search_memories_handler))
        .route("/memories/{id}", delete(delete_memory_handler))
        .route("/memories/compact", post(compact_memories_handler))
        // Key management endpoints (D-18, D-19)
        .route("/keys", post(create_key_handler).get(list_keys_handler))
        .route("/keys/{id}", delete(revoke_key_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public routes: no auth middleware (per D-07, D-08)
    let public = Router::new()
        .route("/health", get(health_handler));

    Router::new()
        .merge(protected)
        .merge(public)
        .with_state(state)
}

/// Centralized scope enforcement for all memory/compaction handlers.
///
/// Returns the effective agent_id to use:
/// - Ok(None): open mode — use whatever the client supplied (no enforcement)
/// - Ok(Some(id)): use this agent_id (either client's or forced from key scope)
/// - Err(Forbidden): scope mismatch
fn enforce_scope(
    auth: Option<&AuthContext>,
    requested: Option<&str>,
) -> Result<Option<String>, crate::error::ApiError> {
    match auth {
        None => Ok(None), // open mode, no enforcement
        Some(ctx) => match &ctx.allowed_agent_id {
            None => Ok(requested.map(str::to_string)), // wildcard key, pass through
            Some(allowed) => match requested {
                None => Ok(Some(allowed.clone())), // missing agent_id, force scope
                Some(req_id) if req_id == allowed.as_str() => Ok(Some(allowed.clone())), // match
                Some(req_id) => Err(crate::error::ApiError::Forbidden(format!(
                    "key scoped to {} cannot access {}", allowed, req_id
                ))), // mismatch -> 403
            },
        },
    }
}

/// GET /health — returns {"status":"ok"} with HTTP 200.
async fn health_handler() -> Json<Value> {
    Json(serde_json::json!({"status": "ok"}))
}

/// POST /memories — creates a new memory and returns 201 Created.
async fn create_memory_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Json(mut body): Json<CreateMemoryRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let auth_ctx = auth.as_ref().map(|Extension(ctx)| ctx);
    let effective = enforce_scope(auth_ctx, body.agent_id.as_deref())?;
    if effective.is_some() {
        body.agent_id = effective;
    }
    let memory = state.service.create_memory(body).await?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::to_value(memory).unwrap())))
}

/// GET /memories/search — semantic search returning ranked results with distance.
async fn search_memories_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Query(mut params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auth_ctx = auth.as_ref().map(|Extension(ctx)| ctx);
    let effective = enforce_scope(auth_ctx, params.agent_id.as_deref())?;
    if effective.is_some() {
        params.agent_id = effective;
    }
    let response = state.service.search_memories(params).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}

/// GET /memories — paginated list of memories with optional filters.
async fn list_memories_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Query(mut params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auth_ctx = auth.as_ref().map(|Extension(ctx)| ctx);
    let effective = enforce_scope(auth_ctx, params.agent_id.as_deref())?;
    if effective.is_some() {
        params.agent_id = effective;
    }
    let response = state.service.list_memories(params).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}

/// DELETE /memories/:id — deletes a memory by ID and returns the deleted object.
async fn delete_memory_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Scope enforcement for scoped keys requires DB lookup (D-12)
    if let Some(Extension(ref ctx)) = auth {
        if let Some(ref allowed_id) = ctx.allowed_agent_id {
            // Fetch memory's agent_id to verify ownership
            match state.service.get_memory_agent_id(&id).await? {
                None => return Err(ApiError::NotFound),
                Some(ref mem_agent_id) if mem_agent_id != allowed_id => {
                    return Err(ApiError::Forbidden(format!(
                        "key scoped to {} cannot access {}", allowed_id, mem_agent_id
                    )));
                }
                Some(_) => {} // Ownership matches, proceed
            }
        }
        // Wildcard key (allowed_agent_id = None): no scope check needed
    }
    // Open mode (auth = None): no scope check needed
    let memory = state.service.delete_memory(id).await?;
    Ok(Json(serde_json::to_value(memory).unwrap()))
}

/// POST /memories/compact — triggers memory compaction for an agent.
async fn compact_memories_handler(
    State(state): State<AppState>,
    auth: Option<Extension<AuthContext>>,
    Json(mut body): Json<CompactRequest>,
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
    // Scope enforcement (D-11): agent_id is required, so enforce_scope gets Some(agent_id)
    let auth_ctx = auth.as_ref().map(|Extension(ctx)| ctx);
    let effective = enforce_scope(auth_ctx, Some(body.agent_id.as_str()))?;
    if let Some(forced) = effective {
        body.agent_id = forced;
    }
    let response = state.compaction.compact(body).await?;
    Ok(Json(serde_json::to_value(response).unwrap()))
}

/// POST /keys — creates a new API key and returns the raw token (shown once).
async fn create_key_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateKeyRequest>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let (api_key, raw_token) = state.key_service.create(body.name, body.agent_id).await
        .map_err(|e| ApiError::Internal(crate::error::MnemonicError::Db(e)))?;
    // CRITICAL: Do NOT log raw_token (PITFALLS.md Auth Pitfall 6)
    tracing::info!(key_id = %api_key.id, agent_id = ?api_key.agent_id, "API key created");
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({
            "key": {
                "id": api_key.id,
                "name": api_key.name,
                "display_id": api_key.display_id,
                "agent_id": api_key.agent_id,
                "created_at": api_key.created_at
            },
            "raw_token": raw_token
        })),
    ))
}

/// GET /keys — returns all key metadata, never including raw token or hashed_key.
async fn list_keys_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let keys = state.key_service.list().await
        .map_err(|e| ApiError::Internal(crate::error::MnemonicError::Db(e)))?;
    Ok(Json(serde_json::json!({
        "keys": keys.iter().map(|k| serde_json::json!({
            "id": k.id,
            "name": k.name,
            "display_id": k.display_id,
            "agent_id": k.agent_id,
            "created_at": k.created_at,
            "revoked_at": k.revoked_at
        })).collect::<Vec<_>>()
    })))
}

/// DELETE /keys/:id — revokes a key (soft delete). Returns confirmation.
async fn revoke_key_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.key_service.revoke(&id).await
        .map_err(|e| ApiError::Internal(crate::error::MnemonicError::Db(e)))?;
    Ok(Json(serde_json::json!({ "revoked": true, "id": id })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthContext;

    fn open_mode() -> Option<AuthContext> {
        None
    }

    fn wildcard_key() -> AuthContext {
        AuthContext {
            key_id: "key-wildcard".to_string(),
            allowed_agent_id: None,
        }
    }

    fn scoped_key(agent_id: &str) -> AuthContext {
        AuthContext {
            key_id: "key-scoped".to_string(),
            allowed_agent_id: Some(agent_id.to_string()),
        }
    }

    /// AUTH-04: open mode (no AuthContext) passes through without enforcement.
    #[test]
    fn test_enforce_scope_open_mode_returns_ok_none() {
        let result = enforce_scope(open_mode().as_ref(), Some("agent-x"));
        assert!(result.is_ok(), "open mode must not return an error");
        assert!(result.unwrap().is_none(), "open mode must return None so caller uses original value");
    }

    /// AUTH-04: wildcard key (allowed_agent_id=None) passes the requested agent_id through unchanged.
    #[test]
    fn test_enforce_scope_wildcard_key_passes_requested_agent_id_through() {
        let ctx = wildcard_key();
        let result = enforce_scope(Some(&ctx), Some("agent-x"));
        assert!(result.is_ok(), "wildcard key must not error");
        assert_eq!(
            result.unwrap(),
            Some("agent-x".to_string()),
            "wildcard key must pass through the client-supplied agent_id"
        );
    }

    /// AUTH-04: scoped key with no requested agent_id forces the key's scope as effective agent_id.
    #[test]
    fn test_enforce_scope_scoped_key_forces_scope_when_no_requested_agent_id() {
        let ctx = scoped_key("agent-A");
        let result = enforce_scope(Some(&ctx), None);
        assert!(result.is_ok(), "scoped key with no requested agent_id must not error");
        assert_eq!(
            result.unwrap(),
            Some("agent-A".to_string()),
            "scoped key must force its own agent_id when none is requested"
        );
    }

    /// AUTH-04: scoped key with matching requested agent_id returns that agent_id.
    #[test]
    fn test_enforce_scope_scoped_key_allows_matching_agent_id() {
        let ctx = scoped_key("agent-A");
        let result = enforce_scope(Some(&ctx), Some("agent-A"));
        assert!(result.is_ok(), "matching agent_id must not return an error");
        assert_eq!(
            result.unwrap(),
            Some("agent-A".to_string()),
            "matching agent_id must be returned as the effective agent_id"
        );
    }

    /// AUTH-04: scoped key with mismatched requested agent_id returns Forbidden error.
    #[test]
    fn test_enforce_scope_scoped_key_rejects_mismatched_agent_id_with_forbidden() {
        let ctx = scoped_key("agent-A");
        let result = enforce_scope(Some(&ctx), Some("agent-B"));
        assert!(result.is_err(), "mismatched agent_id must return an error");
        match result.unwrap_err() {
            ApiError::Forbidden(detail) => {
                assert!(
                    detail.contains("agent-A"),
                    "Forbidden detail must mention the allowed agent, got: {}",
                    detail
                );
                assert!(
                    detail.contains("agent-B"),
                    "Forbidden detail must mention the requested agent, got: {}",
                    detail
                );
            }
            other => panic!("expected ApiError::Forbidden, got {:?}", other),
        }
    }
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
