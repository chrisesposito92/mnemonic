//! Authentication module — API key management and request authorization.
//!
//! Phase 10 foundation: struct definitions and `count_active_keys()`.
//! Phase 11 fills in KeyService methods. Phase 12 adds middleware.

use std::sync::Arc;
use tokio_rusqlite::Connection;

/// A row from the `api_keys` table.
#[allow(dead_code)] // No callers in Phase 10 — used starting Phase 11
#[derive(Debug, Clone)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub display_id: String,
    pub agent_id: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

/// Per-request authentication result, injected into request extensions by the auth middleware.
#[allow(dead_code)] // No callers in Phase 10 — used starting Phase 12
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub key_id: String,
    pub allowed_agent_id: Option<String>,
}

/// Business logic for API key management.
pub struct KeyService {
    conn: Arc<Connection>,
}

impl KeyService {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Counts active (non-revoked) keys in the database.
    /// Returns 0 if the table is empty (open mode).
    /// Used by the startup log to determine auth mode.
    pub async fn count_active_keys(&self) -> Result<i64, crate::error::DbError> {
        self.conn
            .call(|c| -> Result<i64, rusqlite::Error> {
                c.query_row(
                    "SELECT COUNT(*) FROM api_keys WHERE revoked_at IS NULL",
                    [],
                    |row| row.get(0),
                )
            })
            .await
            .map_err(crate::error::DbError::from)
    }

    /// Creates a new API key with the given name and optional agent_id scope.
    /// Returns the persisted ApiKey and the raw token (shown once, never stored).
    pub async fn create(
        &self,
        _name: String,
        _agent_id: Option<String>,
    ) -> Result<(ApiKey, String), crate::error::DbError> {
        todo!("Phase 11: KeyService::create")
    }

    /// Lists all API keys (active and revoked).
    pub async fn list(&self) -> Result<Vec<ApiKey>, crate::error::DbError> {
        todo!("Phase 11: KeyService::list")
    }

    /// Revokes a key by ID. Sets revoked_at to current timestamp.
    /// Idempotent — revoking an already-revoked key is a no-op.
    pub async fn revoke(&self, _id: &str) -> Result<(), crate::error::DbError> {
        todo!("Phase 11: KeyService::revoke")
    }

    /// Validates a raw token against stored hashes.
    /// Returns AuthContext on success, or an error if the token is invalid/revoked.
    pub async fn validate(&self, _raw_token: &str) -> Result<AuthContext, crate::error::DbError> {
        todo!("Phase 11: KeyService::validate")
    }
}
