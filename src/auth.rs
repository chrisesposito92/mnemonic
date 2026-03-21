//! Authentication module — API key management and request authorization.
//!
//! Phase 10 foundation: struct definitions and `count_active_keys()`.
//! Phase 11 fills in KeyService methods. Phase 12 adds middleware.

use std::sync::Arc;
use tokio_rusqlite::Connection;
use constant_time_eq::constant_time_eq_32;
use rand::rand_core::{OsRng, TryRngCore};

/// A row from the `api_keys` table.
#[allow(dead_code)] // Phase 12: used by CLI (Phase 14) and HTTP handlers (Phase 13)
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
#[allow(dead_code)] // Phase 12: auth middleware
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub key_id: String,
    pub allowed_agent_id: Option<String>,
}

/// Business logic for API key management.
pub struct KeyService {
    conn: Arc<Connection>,
}

/// Generates a cryptographically random API key token: "mnk_" + 64 hex chars.
fn generate_raw_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).expect("OsRng entropy unavailable");
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("mnk_{}", hex)
}

/// Hashes a raw token string with BLAKE3, returning the 32-byte hash.
fn hash_token(raw: &str) -> blake3::Hash {
    blake3::hash(raw.as_bytes())
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
        name: String,
        agent_id: Option<String>,
    ) -> Result<(ApiKey, String), crate::error::DbError> {
        let raw_token = generate_raw_token();
        let hash = hash_token(&raw_token);
        let hashed_key = hash.to_hex().to_string();
        let display_id = hashed_key[..8].to_string();
        let id = uuid::Uuid::now_v7().to_string();

        let id_clone = id.clone();
        let name_clone = name.clone();
        let hashed_key_clone = hashed_key.clone();
        let agent_id_clone = agent_id.clone();

        self.conn
            .call(move |c| -> Result<(), rusqlite::Error> {
                c.execute(
                    "INSERT INTO api_keys (id, name, display_id, hashed_key, agent_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![id_clone, name_clone, display_id, hashed_key_clone, agent_id_clone],
                )?;
                Ok(())
            })
            .await
            .map_err(crate::error::DbError::from)?;

        let id_clone2 = id.clone();
        let api_key = self.conn
            .call(move |c| -> Result<ApiKey, rusqlite::Error> {
                c.query_row(
                    "SELECT id, name, display_id, agent_id, created_at, revoked_at FROM api_keys WHERE id = ?1",
                    rusqlite::params![id_clone2],
                    |row| {
                        Ok(ApiKey {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            display_id: row.get(2)?,
                            agent_id: row.get(3)?,
                            created_at: row.get(4)?,
                            revoked_at: row.get(5)?,
                        })
                    },
                )
            })
            .await
            .map_err(crate::error::DbError::from)?;

        Ok((api_key, raw_token))
    }

    /// Lists all API keys (active and revoked).
    pub async fn list(&self) -> Result<Vec<ApiKey>, crate::error::DbError> {
        self.conn
            .call(|c| -> Result<Vec<ApiKey>, rusqlite::Error> {
                let mut stmt = c.prepare(
                    "SELECT id, name, display_id, agent_id, created_at, revoked_at FROM api_keys ORDER BY created_at DESC, id DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(ApiKey {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        display_id: row.get(2)?,
                        agent_id: row.get(3)?,
                        created_at: row.get(4)?,
                        revoked_at: row.get(5)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(crate::error::DbError::from)
    }

    /// Revokes a key by ID. Sets revoked_at to current timestamp.
    /// Idempotent — revoking an already-revoked key is a no-op.
    pub async fn revoke(&self, id: &str) -> Result<(), crate::error::DbError> {
        let id_owned = id.to_string();
        self.conn
            .call(move |c| -> Result<(), rusqlite::Error> {
                c.execute(
                    "UPDATE api_keys SET revoked_at = CURRENT_TIMESTAMP WHERE id = ?1",
                    rusqlite::params![id_owned],
                )?;
                Ok(())
            })
            .await
            .map_err(crate::error::DbError::from)
    }

    /// Validates a raw token against stored hashes.
    /// Returns AuthContext on success, or an error if the token is invalid/revoked.
    pub async fn validate(&self, raw_token: &str) -> Result<AuthContext, crate::error::DbError> {
        let incoming_hash = hash_token(raw_token);
        let incoming_hex = incoming_hash.to_hex().to_string();
        let incoming_bytes: [u8; 32] = *incoming_hash.as_bytes();

        self.conn
            .call(move |c| -> Result<AuthContext, rusqlite::Error> {
                let result = c.query_row(
                    "SELECT id, hashed_key, agent_id FROM api_keys WHERE hashed_key = ?1 AND revoked_at IS NULL",
                    rusqlite::params![incoming_hex],
                    |row| {
                        let key_id: String = row.get(0)?;
                        let stored_hex: String = row.get(1)?;
                        let agent_id: Option<String> = row.get(2)?;
                        Ok((key_id, stored_hex, agent_id))
                    },
                );

                match result {
                    Ok((key_id, stored_hex, agent_id)) => {
                        let stored_hash = blake3::Hash::from_hex(&stored_hex)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                        let stored_bytes: &[u8; 32] = stored_hash.as_bytes();
                        if constant_time_eq_32(&incoming_bytes, stored_bytes) {
                            Ok(AuthContext {
                                key_id,
                                allowed_agent_id: agent_id,
                            })
                        } else {
                            Err(rusqlite::Error::QueryReturnedNoRows)
                        }
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        Err(rusqlite::Error::QueryReturnedNoRows)
                    }
                    Err(e) => Err(e),
                }
            })
            .await
            .map_err(crate::error::DbError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    async fn test_key_service() -> KeyService {
        crate::db::register_sqlite_vec();
        let config = crate::config::Config {
            port: 0,
            db_path: ":memory:".to_string(),
            embedding_provider: "local".to_string(),
            openai_api_key: None,
            ..Default::default()
        };
        let conn = crate::db::open(&config).await.unwrap();
        KeyService::new(Arc::new(conn))
    }

    #[test]
    fn test_generate_raw_token() {
        let token = generate_raw_token();
        assert!(token.starts_with("mnk_"), "token must start with mnk_");
        assert_eq!(token.len(), 68, "token must be 68 chars (4 prefix + 64 hex)");
        let hex_part = &token[4..];
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit()),
            "token suffix must be hex chars"
        );
    }

    #[tokio::test]
    async fn test_create_returns_raw_token() {
        let ks = test_key_service().await;
        let (api_key, raw_token) = ks.create("my-key".to_string(), None).await.unwrap();
        assert!(raw_token.starts_with("mnk_"), "raw_token must start with mnk_");
        assert_eq!(raw_token.len(), 68);
        assert!(!api_key.id.is_empty(), "api_key.id must not be empty");
    }

    #[tokio::test]
    async fn test_create_stores_hash_not_raw() {
        let ks = test_key_service().await;
        let (api_key, raw_token) = ks.create("hash-test".to_string(), None).await.unwrap();

        // Query hashed_key directly from DB
        let id_clone = api_key.id.clone();
        let hashed_key: String = ks
            .conn
            .call(move |c| {
                c.query_row(
                    "SELECT hashed_key FROM api_keys WHERE id = ?1",
                    rusqlite::params![id_clone],
                    |row| row.get(0),
                )
            })
            .await
            .unwrap();

        assert_ne!(hashed_key, raw_token, "hashed_key must not equal raw_token");
        assert_eq!(hashed_key.len(), 64, "hashed_key must be 64 hex chars");
        assert!(
            hashed_key.chars().all(|c| c.is_ascii_hexdigit()),
            "hashed_key must be valid hex"
        );
    }

    #[tokio::test]
    async fn test_display_id_is_hash_derived() {
        let ks = test_key_service().await;
        let (api_key, raw_token) = ks.create("display-test".to_string(), None).await.unwrap();

        let expected_display_id = &blake3::hash(raw_token.as_bytes()).to_hex().to_string()[..8];
        assert_eq!(
            api_key.display_id, expected_display_id,
            "display_id must be first 8 chars of BLAKE3 hash"
        );
        // Verify it is NOT derived from raw token prefix
        assert_ne!(
            api_key.display_id,
            &raw_token[4..12],
            "display_id must not be raw_token[4..12]"
        );
    }

    #[tokio::test]
    async fn test_create_with_name_and_scope() {
        let ks = test_key_service().await;
        let (api_key, _raw_token) = ks
            .create("test-key".to_string(), Some("agent-x".to_string()))
            .await
            .unwrap();
        assert_eq!(api_key.name, "test-key");
        assert_eq!(api_key.agent_id, Some("agent-x".to_string()));
    }

    #[tokio::test]
    async fn test_list_returns_all_keys() {
        let ks = test_key_service().await;
        let (key1, _) = ks.create("key-1".to_string(), None).await.unwrap();
        let (key2, _) = ks.create("key-2".to_string(), None).await.unwrap();
        ks.revoke(&key1.id).await.unwrap();

        let keys = ks.list().await.unwrap();
        assert_eq!(keys.len(), 2, "list must return both active and revoked keys");
        // Most recently created should appear first (ORDER BY created_at DESC)
        assert_eq!(keys[0].id, key2.id, "newest key should be first");
    }

    #[tokio::test]
    async fn test_revoke_prevents_validate() {
        let ks = test_key_service().await;
        let (api_key, raw_token) = ks.create("revoke-test".to_string(), None).await.unwrap();

        // Validate succeeds before revocation
        let auth = ks.validate(&raw_token).await;
        assert!(auth.is_ok(), "validate must succeed before revocation");

        // Revoke the key
        ks.revoke(&api_key.id).await.unwrap();

        // Validate fails after revocation
        let auth_after = ks.validate(&raw_token).await;
        assert!(auth_after.is_err(), "validate must fail after revocation");
    }

    #[tokio::test]
    async fn test_revoke_idempotent() {
        let ks = test_key_service().await;
        // Revoking a non-existent ID must return Ok(())
        let result = ks.revoke("nonexistent-id").await;
        assert!(result.is_ok(), "revoke of non-existent key must return Ok");
    }

    #[tokio::test]
    async fn test_validate_returns_auth_context() {
        let ks = test_key_service().await;
        let (api_key, raw_token) = ks
            .create("auth-ctx-test".to_string(), Some("agent-x".to_string()))
            .await
            .unwrap();

        let auth_context = ks.validate(&raw_token).await.unwrap();
        assert_eq!(auth_context.key_id, api_key.id);
        assert_eq!(auth_context.allowed_agent_id, Some("agent-x".to_string()));
    }

    #[tokio::test]
    async fn test_validate_rejects_wrong_token() {
        let ks = test_key_service().await;
        let _ = ks.create("wrong-token-test".to_string(), None).await.unwrap();

        let wrong_token = "mnk_0000000000000000000000000000000000000000000000000000000000000000";
        let result = ks.validate(wrong_token).await;
        assert!(result.is_err(), "validate must reject wrong token");
    }

    #[tokio::test]
    async fn test_validate_rejects_revoked_key() {
        let ks = test_key_service().await;
        let (api_key, raw_token) = ks.create("revoked-val-test".to_string(), None).await.unwrap();
        ks.revoke(&api_key.id).await.unwrap();

        let result = ks.validate(&raw_token).await;
        assert!(result.is_err(), "validate must reject revoked key");
    }
}
