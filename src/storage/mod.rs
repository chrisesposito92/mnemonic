pub mod sqlite;
pub use sqlite::SqliteBackend;

#[cfg(feature = "backend-qdrant")]
pub mod qdrant;
#[cfg(feature = "backend-qdrant")]
pub use qdrant::QdrantBackend;

#[cfg(feature = "backend-postgres")]
pub mod postgres;
#[cfg(feature = "backend-postgres")]
pub use postgres::PostgresBackend;

use async_trait::async_trait;
use crate::config::Config;
use crate::error::{ApiError, MnemonicError, ConfigError};
use crate::service::{Memory, ListResponse, SearchResponse, ListParams, SearchParams};
use std::sync::Arc;
use tokio_rusqlite::Connection;

// ──────────────────────────────────────────────────────────────────────────────
// Shared input types accepted by StorageBackend methods
// ──────────────────────────────────────────────────────────────────────────────

/// Request to store a new memory. Embedding is pre-computed by the caller (per D-09).
pub struct StoreRequest {
    pub id: String,
    pub content: String,
    pub agent_id: String,
    pub session_id: String,
    pub tags: Vec<String>,
    pub embedding_model: String,
    pub embedding: Vec<f32>,
}

/// Public version of the compaction-internal CandidateMemory struct.
/// Used by fetch_candidates to return embeddings alongside metadata.
pub struct CandidateRecord {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub embedding: Vec<f32>,
}

/// Request to write a single compaction result atomically:
/// insert a merged memory and delete its source memories in one transaction.
pub struct MergedMemoryRequest {
    pub new_id: String,
    pub agent_id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub embedding_model: String,
    pub created_at: String,
    pub source_ids: Vec<String>,
    pub embedding: Vec<f32>,
}

// ──────────────────────────────────────────────────────────────────────────────
// StorageBackend trait
// ──────────────────────────────────────────────────────────────────────────────

/// Abstraction over all persistent memory storage backends.
///
/// Implementations must be `Send + Sync` so they can be shared across async tasks
/// via `Arc<dyn StorageBackend>`. All methods receive pre-computed embeddings;
/// embedding computation is the responsibility of the caller (per D-09).
///
/// Distance semantics: lower values mean higher similarity (per D-02).
/// Backends that natively use higher-is-better scores (e.g. Qdrant) must convert
/// via `1.0 - score` before returning results.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store a memory with a pre-computed embedding. Atomic dual-table write.
    async fn store(&self, req: StoreRequest) -> Result<Memory, ApiError>;

    /// Get a single memory by ID. Returns None if not found.
    async fn get_by_id(&self, id: &str) -> Result<Option<Memory>, ApiError>;

    /// List memories with filtering and pagination.
    async fn list(&self, params: ListParams) -> Result<ListResponse, ApiError>;

    /// Semantic search using a pre-computed query embedding.
    /// Results MUST use lower-is-better distance semantics (D-02).
    async fn search(&self, embedding: Vec<f32>, params: SearchParams) -> Result<SearchResponse, ApiError>;

    /// Delete a memory by ID. Returns the deleted memory or NotFound.
    async fn delete(&self, id: &str) -> Result<Memory, ApiError>;

    /// Fetch compaction candidates with embeddings for an agent.
    /// Returns (candidates, truncated) where truncated=true if more exist beyond max_candidates.
    async fn fetch_candidates(&self, agent_id: &str, max_candidates: u32) -> Result<(Vec<CandidateRecord>, bool), ApiError>;

    /// Atomically insert a merged memory and delete its source memories.
    /// This is a single transaction: insert new + delete sources.
    /// MergedMemoryRequest.source_ids carries the IDs to delete in the same transaction.
    async fn write_compaction_result(&self, req: MergedMemoryRequest) -> Result<Memory, ApiError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// Backend factory
// ──────────────────────────────────────────────────────────────────────────────

/// Creates the appropriate StorageBackend based on config.storage_provider.
///
/// Accepts an Arc<Connection> for the SQLite backend. Non-sqlite backends
/// ignore this parameter and construct their own connections from config.
///
/// For "qdrant" and "postgres" backends, the corresponding feature flag must be
/// enabled at compile time (--features backend-qdrant / --features backend-postgres).
/// If the feature is not enabled, returns a clear error with the required flag name.
pub async fn create_backend(
    config: &Config,
    sqlite_conn: Arc<Connection>,
) -> Result<Arc<dyn StorageBackend>, ApiError> {
    match config.storage_provider.as_str() {
        "sqlite" => {
            Ok(Arc::new(SqliteBackend::new(sqlite_conn)))
        }
        "qdrant" => {
            #[cfg(feature = "backend-qdrant")]
            {
                let backend = qdrant::QdrantBackend::new(config).await?;
                return Ok(Arc::new(backend));
            }
            #[cfg(not(feature = "backend-qdrant"))]
            {
                Err(ApiError::Internal(MnemonicError::Config(ConfigError::Load(
                    "qdrant backend requires building with --features backend-qdrant".to_string()
                ))))
            }
        }
        "postgres" => {
            #[cfg(feature = "backend-postgres")]
            {
                let backend = postgres::PostgresBackend::new(config).await?;
                return Ok(Arc::new(backend));
            }
            #[cfg(not(feature = "backend-postgres"))]
            {
                Err(ApiError::Internal(MnemonicError::Config(ConfigError::Load(
                    "postgres backend requires building with --features backend-postgres".to_string()
                ))))
            }
        }
        other => {
            Err(ApiError::Internal(MnemonicError::Config(ConfigError::Load(
                format!(
                    "unknown storage_provider {:?}: expected \"sqlite\", \"qdrant\", or \"postgres\"",
                    other
                )
            ))))
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Confirms StorageBackend is dyn-compatible (object-safe).
    #[allow(dead_code)]
    fn _assert_object_safe(_: &dyn StorageBackend) {}

    /// Confirms Arc<dyn StorageBackend> compiles (Send + Sync + object-safe).
    #[allow(dead_code)]
    fn _takes_backend(_: Arc<dyn StorageBackend>) {}

    #[test]
    fn test_trait_object_compiles() {
        // Compile-time proof: if this test file compiles, the trait is dyn-compatible.
        // The functions above are the actual assertions.
    }

    #[test]
    fn test_storage_backend_send_sync() {
        // Arc<dyn StorageBackend> requires both Send and Sync.
        // The _takes_backend function above proves this at compile time.
    }

    // ──────────────────────────────────────────────────────────────────────
    // create_backend() factory tests
    // ──────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_backend_sqlite() {
        crate::db::register_sqlite_vec();
        let conn = tokio_rusqlite::Connection::open_in_memory().await.unwrap();
        let config = crate::config::Config::default(); // storage_provider = "sqlite"
        let result = create_backend(&config, Arc::new(conn)).await;
        assert!(result.is_ok(), "sqlite backend should succeed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_create_backend_qdrant_no_feature() {
        // When built without backend-qdrant feature, should return an error
        // mentioning "backend-qdrant"
        let conn = tokio_rusqlite::Connection::open_in_memory().await.unwrap();
        let config = crate::config::Config {
            storage_provider: "qdrant".to_string(),
            qdrant_url: Some("http://localhost:6334".to_string()),
            ..crate::config::Config::default()
        };
        let result = create_backend(&config, Arc::new(conn)).await;
        // Without backend-qdrant feature, this MUST be an error
        #[cfg(not(feature = "backend-qdrant"))]
        {
            assert!(result.is_err(), "qdrant backend should error without backend-qdrant feature");
            let err_str = format!("{:?}", result.err());
            assert!(
                err_str.contains("backend-qdrant"),
                "error should mention backend-qdrant, got: {}",
                err_str
            );
        }
        // With backend-qdrant feature, behavior defined in Phase 23
        #[cfg(feature = "backend-qdrant")]
        {
            // Feature not compiled in this phase — test ensures coverage
            let _ = result;
        }
    }

    #[tokio::test]
    async fn test_create_backend_postgres_no_feature() {
        // When built without backend-postgres feature, should return an error
        // mentioning "backend-postgres"
        let conn = tokio_rusqlite::Connection::open_in_memory().await.unwrap();
        let config = crate::config::Config {
            storage_provider: "postgres".to_string(),
            postgres_url: Some("postgres://localhost/mnemonic".to_string()),
            ..crate::config::Config::default()
        };
        let result = create_backend(&config, Arc::new(conn)).await;
        // Without backend-postgres feature, this MUST be an error
        #[cfg(not(feature = "backend-postgres"))]
        {
            assert!(result.is_err(), "postgres backend should error without backend-postgres feature");
            let err_str = format!("{:?}", result.err());
            assert!(
                err_str.contains("backend-postgres"),
                "error should mention backend-postgres, got: {}",
                err_str
            );
        }
        // With backend-postgres feature, behavior defined in Phase 24
        #[cfg(feature = "backend-postgres")]
        {
            let _ = result;
        }
    }

    #[tokio::test]
    async fn test_create_backend_unknown_provider() {
        let conn = tokio_rusqlite::Connection::open_in_memory().await.unwrap();
        let config = crate::config::Config {
            storage_provider: "redis".to_string(),
            ..crate::config::Config::default()
        };
        let result = create_backend(&config, Arc::new(conn)).await;
        assert!(result.is_err(), "unknown storage_provider should return error");
        let err_str = format!("{:?}", result.err());
        assert!(
            err_str.contains("unknown storage_provider"),
            "error should mention unknown storage_provider, got: {}",
            err_str
        );
    }
}
