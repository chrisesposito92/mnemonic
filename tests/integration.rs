use mnemonic::compaction::{CompactionService, CompactRequest};
use mnemonic::embedding::{EmbeddingEngine, LocalEngine};
use mnemonic::summarization::MockSummarizer;
use std::sync::{Arc, Once, OnceLock};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use mnemonic::server::{AppState, build_router};
use mnemonic::service::MemoryService;

static INIT: Once = Once::new();

/// Shared LocalEngine instance loaded once for all embedding tests.
///
/// Avoids parallel HuggingFace Hub lock contention when multiple tests call
/// LocalEngine::new() concurrently. The first test to run loads the model;
/// all subsequent tests reuse the same instance.
static LOCAL_ENGINE: OnceLock<Arc<LocalEngine>> = OnceLock::new();

fn local_engine() -> Arc<LocalEngine> {
    Arc::clone(LOCAL_ENGINE.get_or_init(|| {
        let engine = LocalEngine::new().expect("LocalEngine::new() should succeed");
        Arc::new(engine)
    }))
}

fn setup() {
    INIT.call_once(|| {
        mnemonic::db::register_sqlite_vec();
    });
}

fn test_config() -> mnemonic::config::Config {
    mnemonic::config::Config {
        port: 0,
        db_path: ":memory:".to_string(),
        embedding_provider: "local".to_string(),
        openai_api_key: None,
        ..Default::default()
    }
}

/// Verifies that the memories table exists after db::open() and contains all 9 required columns.
#[tokio::test]
async fn test_schema_created() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    // Verify memories table exists
    let table_exists = conn
        .call(|c| -> Result<bool, rusqlite::Error> {
            let mut stmt = c.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='memories'",
            )?;
            let mut rows = stmt.query([])?;
            Ok(rows.next()?.is_some())
        })
        .await
        .unwrap();

    assert!(table_exists, "memories table should exist");

    // Verify all 9 columns are present
    let column_names: Vec<String> = conn
        .call(|c| -> Result<Vec<String>, rusqlite::Error> {
            let mut stmt = c.prepare("PRAGMA table_info(memories)")?;
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(names)
        })
        .await
        .unwrap();

    let expected_columns = [
        "id",
        "content",
        "agent_id",
        "session_id",
        "tags",
        "embedding_model",
        "created_at",
        "updated_at",
        "source_ids",
    ];

    for col in &expected_columns {
        assert!(
            column_names.iter().any(|c| c == col),
            "memories table should have column '{}'",
            col
        );
    }

    assert_eq!(column_names.len(), 9, "memories table should have exactly 9 columns");
}

/// Verifies that WAL journal mode is active after db::open().
///
/// Note: SQLite in-memory databases do not support WAL mode — they always use "memory" journal
/// mode. This test uses a temporary file-based database to confirm that the WAL PRAGMA executes
/// correctly against a real file path.
#[tokio::test]
async fn test_wal_mode() {
    setup();
    let tmp_dir = std::env::temp_dir();
    let db_file = tmp_dir.join(format!("mnemonic_test_wal_{}.db", std::process::id()));
    let db_path = db_file.to_str().unwrap().to_string();

    let config = mnemonic::config::Config {
        port: 0,
        db_path: db_path.clone(),
        embedding_provider: "local".to_string(),
        openai_api_key: None,
        ..Default::default()
    };

    let conn = mnemonic::db::open(&config).await.unwrap();

    let journal_mode: String = conn
        .call(|c| -> Result<String, rusqlite::Error> {
            let mut stmt = c.prepare("PRAGMA journal_mode")?;
            let mode: String = stmt.query_row([], |row| row.get(0))?;
            Ok(mode)
        })
        .await
        .unwrap();

    // Clean up temp file
    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(format!("{}-wal", db_path));
    let _ = std::fs::remove_file(format!("{}-shm", db_path));

    assert_eq!(journal_mode, "wal", "journal mode should be WAL for file-based database");
}

/// Verifies that the vec_memories virtual table exists after db::open().
#[tokio::test]
async fn test_vec_memories_exists() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    let table_exists = conn
        .call(|c| -> Result<bool, rusqlite::Error> {
            let mut stmt = c.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='vec_memories'",
            )?;
            let mut rows = stmt.query([])?;
            Ok(rows.next()?.is_some())
        })
        .await
        .unwrap();

    assert!(table_exists, "vec_memories virtual table should exist");
}

/// Verifies that the embedding_model column exists in memories with type TEXT.
#[tokio::test]
async fn test_embedding_model_column() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    let col_type: Option<String> = conn
        .call(|c| -> Result<Option<String>, rusqlite::Error> {
            let mut stmt = c.prepare("PRAGMA table_info(memories)")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let name: String = row.get(1)?;
                if name == "embedding_model" {
                    let col_type: String = row.get(2)?;
                    return Ok(Some(col_type));
                }
            }
            Ok(None)
        })
        .await
        .unwrap();

    assert!(
        col_type.is_some(),
        "embedding_model column should exist in memories table"
    );
    assert_eq!(
        col_type.unwrap(),
        "TEXT",
        "embedding_model column should have type TEXT"
    );
}

/// Verifies that the compact_runs table exists after db::open() and contains all 10 required columns.
#[tokio::test]
async fn test_compact_runs_exists() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    let column_names: Vec<String> = conn
        .call(|c| -> Result<Vec<String>, rusqlite::Error> {
            let mut stmt = c.prepare("PRAGMA table_info(compact_runs)")?;
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(names)
        })
        .await
        .unwrap();

    let expected_columns = [
        "id", "agent_id", "started_at", "completed_at",
        "clusters_found", "memories_merged", "memories_created",
        "dry_run", "threshold", "status",
    ];

    for col in &expected_columns {
        assert!(
            column_names.iter().any(|c| c == col),
            "compact_runs table should have column '{}'",
            col
        );
    }

    assert_eq!(column_names.len(), 10, "compact_runs table should have exactly 10 columns");
}

/// Verifies that idx_compact_runs_agent_id index exists on the compact_runs table after db::open().
#[tokio::test]
async fn test_compact_runs_agent_id_index() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    let index_names: Vec<String> = conn
        .call(|c| -> Result<Vec<String>, rusqlite::Error> {
            let mut stmt = c.prepare("PRAGMA index_list(compact_runs)")?;
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(names)
        })
        .await
        .unwrap();

    assert!(
        index_names.iter().any(|n| n == "idx_compact_runs_agent_id"),
        "compact_runs table should have idx_compact_runs_agent_id index, found: {:?}",
        index_names
    );
}

/// Verifies that db::open() is idempotent — calling twice on the same database produces no error.
#[tokio::test]
async fn test_db_open_idempotent() {
    setup();
    let tmp_dir = std::env::temp_dir();
    let db_file = tmp_dir.join(format!("mnemonic_test_idempotent_{}.db", std::process::id()));
    let db_path = db_file.to_str().unwrap().to_string();

    let config = mnemonic::config::Config {
        port: 0,
        db_path: db_path.clone(),
        embedding_provider: "local".to_string(),
        openai_api_key: None,
        ..Default::default()
    };

    // First open — creates schema
    let conn1 = mnemonic::db::open(&config).await.unwrap();
    drop(conn1);

    // Second open — must not error (all DDL is IF NOT EXISTS)
    let conn2 = mnemonic::db::open(&config).await.unwrap();
    drop(conn2);

    // Clean up
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(format!("{}-wal", db_path));
    let _ = std::fs::remove_file(format!("{}-shm", db_path));
}

/// INFRA-01: Verifies that the api_keys table exists after db::open() with all 7 required columns.
#[tokio::test]
async fn test_api_keys_table_created() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    let column_names: Vec<String> = conn
        .call(|c| -> Result<Vec<String>, rusqlite::Error> {
            let mut stmt = c.prepare("PRAGMA table_info(api_keys)")?;
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(names)
        })
        .await
        .unwrap();

    let expected_columns = [
        "id", "name", "display_id", "hashed_key",
        "agent_id", "created_at", "revoked_at",
    ];

    for col in &expected_columns {
        assert!(
            column_names.iter().any(|c| c == col),
            "api_keys table should have column '{}', found: {:?}",
            col,
            column_names
        );
    }

    assert_eq!(column_names.len(), 7, "api_keys table should have exactly 7 columns");
}

/// INFRA-01: Verifies that idx_api_keys_agent_id index exists.
/// Note: hashed_key has a UNIQUE constraint which creates an implicit auto-index
/// (named sqlite_autoindex_api_keys_1), so we only check the explicit agent_id index.
#[tokio::test]
async fn test_api_keys_indexes() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    let index_names: Vec<String> = conn
        .call(|c| -> Result<Vec<String>, rusqlite::Error> {
            let mut stmt = c.prepare("PRAGMA index_list(api_keys)")?;
            let names = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(names)
        })
        .await
        .unwrap();

    assert!(
        index_names.iter().any(|n| n == "idx_api_keys_agent_id"),
        "api_keys should have idx_api_keys_agent_id index, found: {:?}",
        index_names
    );

    // Verify the UNIQUE constraint auto-index exists for hashed_key
    assert!(
        index_names.iter().any(|n| n.contains("autoindex") || n == "sqlite_autoindex_api_keys_1"),
        "api_keys should have an auto-index from the UNIQUE constraint on hashed_key, found: {:?}",
        index_names
    );
}

/// INFRA-01: Verifies that db::open() with api_keys DDL is idempotent — runs twice without error.
#[tokio::test]
async fn test_api_keys_migration_idempotent() {
    setup();
    let tmp_dir = std::env::temp_dir();
    let db_file = tmp_dir.join(format!("mnemonic_test_api_keys_idempotent_{}.db", std::process::id()));
    let db_path = db_file.to_str().unwrap().to_string();

    let config = mnemonic::config::Config {
        port: 0,
        db_path: db_path.clone(),
        embedding_provider: "local".to_string(),
        openai_api_key: None,
        ..Default::default()
    };

    // First open — creates api_keys table
    let conn1 = mnemonic::db::open(&config).await.unwrap();
    drop(conn1);

    // Second open — must not error (CREATE TABLE IF NOT EXISTS)
    let conn2 = mnemonic::db::open(&config).await.unwrap();
    drop(conn2);

    // Clean up
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(format!("{}-wal", db_path));
    let _ = std::fs::remove_file(format!("{}-shm", db_path));
}

/// INFRA-03: count_active_keys() returns 0 on a fresh database (open mode).
#[tokio::test]
async fn test_count_active_keys_empty_db() {
    setup();
    let config = test_config();
    let conn = Arc::new(mnemonic::db::open(&config).await.unwrap());
    let key_service = mnemonic::auth::KeyService::new(conn);
    let count = key_service.count_active_keys().await.unwrap();
    assert_eq!(count, 0, "empty DB should report 0 active keys (open mode)");
}

/// D-07, D-08: ApiError::Unauthorized produces 401 with structured JSON body.
#[tokio::test]
async fn test_unauthorized_response_shape() {
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    let error = mnemonic::error::ApiError::Unauthorized("test reason".to_string());
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "unauthorized");
    assert_eq!(json["auth_mode"], "active");
    assert_eq!(json["hint"], "Provide Authorization: Bearer mnk_...");
}

/// Verifies that db::open() works in an async context and supports insert + query via conn.call().
#[tokio::test]
async fn test_db_open_async() {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();

    // Insert a test row
    conn.call(|c| -> Result<(), rusqlite::Error> {
        c.execute(
            "INSERT INTO memories (id, content) VALUES ('test-id', 'test content')",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    // Query it back
    let content: String = conn
        .call(|c| -> Result<String, rusqlite::Error> {
            let mut stmt =
                c.prepare("SELECT content FROM memories WHERE id = 'test-id'")?;
            let content: String = stmt.query_row([], |row| row.get(0))?;
            Ok(content)
        })
        .await
        .unwrap();

    assert_eq!(content, "test content", "inserted content should round-trip correctly");
}

/// Verifies that LocalEngine::embed returns a 384-dimensional vector.
/// Requires model to be downloaded (happens on first run).
#[tokio::test]
async fn test_local_embedding_384_dimensions() {
    let engine = tokio::task::spawn_blocking(local_engine).await.unwrap();
    let embedding = engine
        .embed("hello world")
        .await
        .expect("embed should succeed");
    assert_eq!(embedding.len(), 384, "embedding should be exactly 384 dimensions");
}

/// Verifies that the embedding vector is L2-normalized (norm ~= 1.0).
#[tokio::test]
async fn test_local_embedding_normalized() {
    let engine = tokio::task::spawn_blocking(local_engine).await.unwrap();
    let embedding = engine
        .embed("test normalization")
        .await
        .expect("embed should succeed");
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.01,
        "L2 norm should be approximately 1.0, got {}",
        norm
    );
}

/// Verifies semantic similarity: related words produce higher cosine similarity
/// than unrelated words. This proves correct pooling and normalization.
#[tokio::test]
async fn test_semantic_similarity() {
    let engine = tokio::task::spawn_blocking(local_engine).await.unwrap();

    let dog = engine.embed("dog").await.unwrap();
    let puppy = engine.embed("puppy").await.unwrap();
    let database = engine.embed("database").await.unwrap();

    let sim_related = cosine_similarity(&dog, &puppy);
    let sim_unrelated = cosine_similarity(&dog, &database);

    assert!(
        sim_related > sim_unrelated,
        "cosine similarity of 'dog'/'puppy' ({:.4}) should be greater than 'dog'/'database' ({:.4})",
        sim_related,
        sim_unrelated
    );
    assert!(
        sim_related > 0.5,
        "cosine similarity of 'dog'/'puppy' should be > 0.5, got {:.4}",
        sim_related
    );
    assert!(
        sim_unrelated < 0.5,
        "cosine similarity of 'dog'/'database' should be < 0.5, got {:.4}",
        sim_unrelated
    );
}

/// Verifies that calling embed() with empty text returns EmbeddingError::EmptyInput.
#[tokio::test]
async fn test_empty_input_error() {
    let engine = tokio::task::spawn_blocking(local_engine).await.unwrap();
    let result = engine.embed("").await;
    assert!(result.is_err(), "empty input should return an error");
    let err = result.unwrap_err();
    assert!(
        format!("{}", err).contains("empty input text"),
        "error message should mention empty input, got: {}",
        err
    );
}

/// Verifies the engine can be called multiple times without reinitializing.
#[tokio::test]
async fn test_embed_reuse() {
    let engine = tokio::task::spawn_blocking(local_engine).await.unwrap();
    let first = engine.embed("first call").await.unwrap();
    let second = engine.embed("second call").await.unwrap();
    assert_eq!(first.len(), 384);
    assert_eq!(second.len(), 384);
    // Embeddings should be different for different inputs
    assert_ne!(first, second, "different inputs should produce different embeddings");
}

/// EMBD-04: Verifies OpenAiEngine returns a valid 384-dim embedding from the live API.
/// Ignored by default — requires MNEMONIC_OPENAI_API_KEY env var.
#[tokio::test]
#[ignore]
async fn test_openai_embedding() {
    use mnemonic::embedding::{OpenAiEngine, EmbeddingEngine};

    let api_key = std::env::var("MNEMONIC_OPENAI_API_KEY")
        .expect("MNEMONIC_OPENAI_API_KEY must be set to run this test");
    let engine = OpenAiEngine::new(api_key);

    let embedding = engine.embed("hello world").await
        .expect("OpenAI embed should succeed");
    assert_eq!(embedding.len(), 384, "OpenAI embedding should be exactly 384 dimensions");

    // Verify L2 normalization (OpenAI returns normalized vectors)
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.05,
        "L2 norm should be approximately 1.0, got {}",
        norm
    );

    // Verify empty input returns error
    let result = engine.embed("").await;
    assert!(result.is_err(), "empty input should return an error");
}

/// Cosine similarity helper for test assertions.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

// ────────────────────────────────────────────────────────────────────────────
// API Integration Test Infrastructure
// ────────────────────────────────────────────────────────────────────────────

/// MockEmbeddingEngine returns deterministic 384-dim vectors based on a hash
/// of the input text, enabling fast, reproducible API integration tests
/// without requiring model downloads.
struct MockEmbeddingEngine;

#[async_trait::async_trait]
impl mnemonic::embedding::EmbeddingEngine for MockEmbeddingEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, mnemonic::error::EmbeddingError> {
        if text.is_empty() {
            return Err(mnemonic::error::EmbeddingError::EmptyInput);
        }
        // Generate a deterministic 384-dim vector from text hash
        let mut embedding = vec![0.0f32; 384];
        let bytes = text.as_bytes();
        for (i, slot) in embedding.iter_mut().enumerate() {
            let mut hash: u32 = 5381;
            for &b in bytes {
                hash = hash.wrapping_mul(33).wrapping_add(b as u32);
            }
            hash = hash.wrapping_mul(31).wrapping_add(i as u32);
            *slot = (hash as f32 % 1000.0) / 1000.0;
        }
        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in embedding.iter_mut() {
                *v /= norm;
            }
        }
        Ok(embedding)
    }
}

/// Creates shared AppState with an in-memory SQLite DB, MockEmbeddingEngine,
/// and MemoryService. The returned service Arc allows inserting test data
/// before routing requests.
async fn build_test_state() -> (AppState, Arc<MemoryService>) {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();
    let db = Arc::new(conn);
    let embedding: Arc<dyn mnemonic::embedding::EmbeddingEngine> = Arc::new(MockEmbeddingEngine);
    let service = Arc::new(MemoryService::new(
        db.clone(),
        embedding.clone(),
        "mock-model".to_string(),
    ));
    let compaction = Arc::new(CompactionService::new(
        db.clone(), embedding.clone(), None, "mock-model".to_string(),
    ));
    let key_service = Arc::new(mnemonic::auth::KeyService::new(db.clone()));
    let state = AppState {
        service: service.clone(),
        compaction,
        key_service,
    };
    (state, service)
}

/// Creates a fully wired axum Router backed by a fresh in-memory DB.
async fn build_test_app() -> axum::Router {
    let (state, _) = build_test_state().await;
    build_router(state)
}

/// Builds a JSON POST/PUT/DELETE request with content-type application/json.
fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

/// Consumes an axum response body and deserializes it as JSON.
async fn response_json(response: axum::http::Response<Body>) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// ────────────────────────────────────────────────────────────────────────────
// API Integration Tests
// ────────────────────────────────────────────────────────────────────────────

/// API-05: GET /health returns 200 with {"status":"ok"}.
#[tokio::test]
async fn test_health() {
    let app = build_test_app().await;
    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["status"], "ok");
}

/// API-01, API-06: POST /memories returns 201 Created with a full memory object.
#[tokio::test]
async fn test_post_memory() {
    let app = build_test_app().await;
    let response = app
        .oneshot(json_request("POST", "/memories", serde_json::json!({
            "content": "The quick brown fox",
            "agent_id": "agent-1",
            "session_id": "sess-1",
            "tags": ["test", "fox"]
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = response_json(response).await;
    assert!(json["id"].is_string(), "response must have string id");
    assert_eq!(json["content"], "The quick brown fox");
    assert_eq!(json["agent_id"], "agent-1");
    assert_eq!(json["session_id"], "sess-1");
    assert_eq!(json["tags"], serde_json::json!(["test", "fox"]));
    assert_eq!(json["embedding_model"], "mock-model");
    assert!(json["created_at"].is_string(), "response must have created_at");
}

/// API-01, API-06: POST /memories with empty content returns 400 with JSON error body.
#[tokio::test]
async fn test_post_memory_validation() {
    let app = build_test_app().await;
    let response = app
        .oneshot(json_request("POST", "/memories", serde_json::json!({
            "content": ""
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert!(json["error"].is_string(), "error response must have error field");
}

/// API-03, AGNT-01: GET /memories returns paginated list with total count;
/// agent_id filter returns only that agent's memories.
#[tokio::test]
async fn test_list_memories() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_state().await;

    // Insert 3 memories: 2 for agent "a1", 1 for agent "a2"
    service.create_memory(CreateMemoryRequest {
        content: "first memory".to_string(),
        agent_id: Some("a1".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "second memory".to_string(),
        agent_id: Some("a1".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "third memory".to_string(),
        agent_id: Some("a2".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // GET /memories?agent_id=a1 -- should return 2
    let app = build_router(state.clone());
    let response = app
        .oneshot(Request::get("/memories?agent_id=a1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 2, "agent a1 should have 2 memories");
    assert_eq!(json["memories"].as_array().unwrap().len(), 2);

    // GET /memories -- should return all 3
    let app = build_router(state.clone());
    let response = app
        .oneshot(Request::get("/memories").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 3, "total should be 3 across all agents");
}

/// API-02: GET /memories/search?q=... returns ranked results with distance field.
#[tokio::test]
async fn test_search_memories() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_state().await;

    service.create_memory(CreateMemoryRequest {
        content: "rust programming language".to_string(),
        agent_id: Some("agent-1".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "cooking recipes for dinner".to_string(),
        agent_id: Some("agent-1".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::get("/memories/search?q=rust+programming")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let memories = json["memories"].as_array().expect("memories should be array");
    assert!(!memories.is_empty(), "search should return at least one result");
    // Each result should have a distance field
    assert!(memories[0]["distance"].is_number(), "each result must have numeric distance");
    assert!(memories[0]["id"].is_string(), "each result must have id");
}

/// API-02, API-06: GET /memories/search without q parameter returns 400.
#[tokio::test]
async fn test_search_missing_q() {
    let app = build_test_app().await;
    let response = app
        .oneshot(Request::get("/memories/search").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert!(json["error"].is_string(), "error response must have error field");
}

/// API-04: DELETE /memories/:id returns 200 with deleted memory object;
/// subsequent GET /memories returns total=0.
#[tokio::test]
async fn test_delete_memory() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_state().await;

    let memory = service.create_memory(CreateMemoryRequest {
        content: "memory to delete".to_string(),
        agent_id: None,
        session_id: None,
        tags: None,
    }).await.unwrap();

    let id = memory.id.clone();

    // DELETE /memories/:id -- should return 200 with deleted object
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/memories/{}", id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["id"], id, "deleted response should contain the memory id");
    assert_eq!(json["content"], "memory to delete");

    // Verify memory no longer exists via GET /memories
    let app = build_router(state.clone());
    let response = app
        .oneshot(Request::get("/memories").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 0, "total should be 0 after deletion");
}

/// API-04, API-06: DELETE /memories/:id for nonexistent id returns 404 with JSON error body.
#[tokio::test]
async fn test_delete_not_found() {
    let app = build_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/memories/nonexistent-id-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = response_json(response).await;
    assert!(json["error"].is_string(), "error response must have error field");
}

/// AGNT-01, AGNT-03: Two agents storing memories with same content retrieve only their own
/// when filtering by agent_id.
#[tokio::test]
async fn test_agent_isolation() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_state().await;

    service.create_memory(CreateMemoryRequest {
        content: "shared content".to_string(),
        agent_id: Some("agent-a".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "shared content".to_string(),
        agent_id: Some("agent-b".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // GET /memories?agent_id=agent-a
    let app = build_router(state.clone());
    let response = app
        .oneshot(Request::get("/memories?agent_id=agent-a").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 1, "agent-a should have exactly 1 memory");
    let memories = json["memories"].as_array().unwrap();
    assert_eq!(memories[0]["agent_id"], "agent-a");

    // GET /memories?agent_id=agent-b
    let app = build_router(state.clone());
    let response = app
        .oneshot(Request::get("/memories?agent_id=agent-b").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 1, "agent-b should have exactly 1 memory");
    let memories = json["memories"].as_array().unwrap();
    assert_eq!(memories[0]["agent_id"], "agent-b");
}

/// AGNT-02: Session filter scopes list retrieval to specific session_id.
#[tokio::test]
async fn test_session_filter() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_state().await;

    service.create_memory(CreateMemoryRequest {
        content: "session one memory".to_string(),
        agent_id: Some("agent-1".to_string()),
        session_id: Some("s1".to_string()),
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "session two memory".to_string(),
        agent_id: Some("agent-1".to_string()),
        session_id: Some("s2".to_string()),
        tags: None,
    }).await.unwrap();

    // GET /memories?session_id=s1 should return only the s1 memory
    let app = build_router(state.clone());
    let response = app
        .oneshot(Request::get("/memories?session_id=s1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 1, "session s1 should have exactly 1 memory");
    let memories = json["memories"].as_array().unwrap();
    assert_eq!(memories[0]["session_id"], "s1");
}

/// AGNT-03: Search with agent_id filter returns only that agent's memories.
#[tokio::test]
async fn test_search_agent_filter() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_state().await;

    service.create_memory(CreateMemoryRequest {
        content: "cats are great pets".to_string(),
        agent_id: Some("x".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "cats are wonderful animals".to_string(),
        agent_id: Some("y".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // GET /memories/search?q=cats&agent_id=x -- all results must belong to agent "x"
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::get("/memories/search?q=cats&agent_id=x")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let memories = json["memories"].as_array().expect("memories should be array");
    assert!(!memories.is_empty(), "search should return at least one result for agent x");
    for m in memories {
        assert_eq!(m["agent_id"], "x", "all search results should belong to agent x");
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Compaction Integration Tests
// ────────────────────────────────────────────────────────────────────────────

/// Creates a CompactionService with MockEmbeddingEngine and optional MockSummarizer,
/// sharing the same in-memory DB as the MemoryService.
async fn build_test_compaction(with_llm: bool) -> (Arc<CompactionService>, Arc<mnemonic::service::MemoryService>) {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();
    let db = Arc::new(conn);
    let embedding: Arc<dyn mnemonic::embedding::EmbeddingEngine> = Arc::new(MockEmbeddingEngine);
    let summarization: Option<Arc<dyn mnemonic::summarization::SummarizationEngine>> = if with_llm {
        Some(Arc::new(MockSummarizer))
    } else {
        None
    };
    let service = Arc::new(mnemonic::service::MemoryService::new(
        db.clone(), embedding.clone(), "mock-model".to_string(),
    ));
    let compaction = Arc::new(CompactionService::new(
        db.clone(), embedding.clone(), summarization, "mock-model".to_string(),
    ));
    (compaction, service)
}

/// DEDUP-01, DEDUP-02, DEDUP-03: Compacting similar memories produces merged memory with correct
/// metadata, and source memories are deleted atomically.
#[tokio::test]
async fn test_compact_atomic_write() {
    use mnemonic::service::CreateMemoryRequest;

    let (compaction, service) = build_test_compaction(false).await;

    // Insert 2 memories with IDENTICAL content (MockEmbeddingEngine produces same vector -> cosine sim = 1.0)
    let m1 = service.create_memory(CreateMemoryRequest {
        content: "the cat sat on the mat".to_string(),
        agent_id: Some("agent-1".to_string()),
        session_id: Some("sess-1".to_string()),
        tags: Some(vec!["animal".to_string(), "cat".to_string()]),
    }).await.unwrap();
    let m2 = service.create_memory(CreateMemoryRequest {
        content: "the cat sat on the mat".to_string(),
        agent_id: Some("agent-1".to_string()),
        session_id: Some("sess-2".to_string()),
        tags: Some(vec!["cat".to_string(), "furniture".to_string()]),
    }).await.unwrap();

    // Compact with very low threshold to guarantee clustering
    let response = compaction.compact(CompactRequest {
        agent_id: "agent-1".to_string(),
        threshold: Some(0.5),
        max_candidates: None,
        dry_run: Some(false),
    }).await.unwrap();

    // Verify response counts
    assert_eq!(response.clusters_found, 1, "should find 1 cluster");
    assert_eq!(response.memories_merged, 2, "should merge 2 memories");
    assert_eq!(response.memories_created, 1, "should create 1 merged memory");
    assert!(!response.run_id.is_empty(), "run_id must not be empty");

    // Verify id_mapping
    assert_eq!(response.id_mapping.len(), 1, "should have 1 cluster mapping");
    let mapping = &response.id_mapping[0];
    assert!(mapping.source_ids.contains(&m1.id), "source_ids must contain m1");
    assert!(mapping.source_ids.contains(&m2.id), "source_ids must contain m2");
    assert!(mapping.new_id.is_some(), "new_id must be Some in non-dry-run");

    // Verify source memories are DELETED
    let list = service.list_memories(mnemonic::service::ListParams {
        agent_id: Some("agent-1".to_string()),
        session_id: None, tag: None, after: None, before: None,
        limit: None, offset: None,
    }).await.unwrap();
    assert_eq!(list.total, 1, "should have exactly 1 memory after compaction (the merged one)");

    // Verify merged memory has correct metadata
    let merged = &list.memories[0];
    assert_eq!(merged.id, mapping.new_id.as_ref().unwrap().clone());
    // Tags should be union: animal, cat, furniture (deduplicated)
    assert!(merged.tags.contains(&"animal".to_string()), "merged tags must contain 'animal'");
    assert!(merged.tags.contains(&"cat".to_string()), "merged tags must contain 'cat'");
    assert!(merged.tags.contains(&"furniture".to_string()), "merged tags must contain 'furniture'");
    // created_at should be earliest (m1 was created first)
    assert_eq!(merged.created_at, m1.created_at, "merged created_at should be earliest source");
}

/// DEDUP-01, DEDUP-03: dry_run returns cluster preview without modifying data.
#[tokio::test]
async fn test_compact_dry_run() {
    use mnemonic::service::CreateMemoryRequest;

    let (compaction, service) = build_test_compaction(false).await;

    // Insert 2 identical-content memories
    service.create_memory(CreateMemoryRequest {
        content: "identical content for dry run".to_string(),
        agent_id: Some("agent-dry".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "identical content for dry run".to_string(),
        agent_id: Some("agent-dry".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();

    // Count before
    let before = service.list_memories(mnemonic::service::ListParams {
        agent_id: Some("agent-dry".to_string()),
        session_id: None, tag: None, after: None, before: None,
        limit: None, offset: None,
    }).await.unwrap();
    assert_eq!(before.total, 2);

    // Compact with dry_run=true
    let response = compaction.compact(CompactRequest {
        agent_id: "agent-dry".to_string(),
        threshold: Some(0.5),
        max_candidates: None,
        dry_run: Some(true),
    }).await.unwrap();

    assert_eq!(response.clusters_found, 1, "dry_run should still find clusters");
    assert_eq!(response.memories_merged, 2);
    // In dry_run, memories_created is 0 — no actual writes performed
    assert_eq!(response.memories_created, 0);
    // new_id should be None in dry_run
    assert!(response.id_mapping[0].new_id.is_none(), "new_id must be None in dry_run");

    // Count after — should be UNCHANGED
    let after = service.list_memories(mnemonic::service::ListParams {
        agent_id: Some("agent-dry".to_string()),
        session_id: None, tag: None, after: None, before: None,
        limit: None, offset: None,
    }).await.unwrap();
    assert_eq!(after.total, 2, "dry_run must not modify memory count");
}

/// DEDUP-01: When no memories exceed similarity threshold, no clusters are formed.
#[tokio::test]
async fn test_compact_no_clusters() {
    use mnemonic::service::CreateMemoryRequest;

    let (compaction, service) = build_test_compaction(false).await;

    // Insert 2 memories with DIFFERENT content (different embeddings)
    service.create_memory(CreateMemoryRequest {
        content: "apples and oranges are fruit".to_string(),
        agent_id: Some("agent-diff".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "quantum physics and black holes".to_string(),
        agent_id: Some("agent-diff".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();

    // Compact with HIGH threshold (0.99) — unlikely to cluster different content
    let response = compaction.compact(CompactRequest {
        agent_id: "agent-diff".to_string(),
        threshold: Some(0.99),
        max_candidates: None,
        dry_run: Some(false),
    }).await.unwrap();

    assert_eq!(response.clusters_found, 0, "different content should not cluster at 0.99 threshold");
    assert_eq!(response.memories_merged, 0);
    assert_eq!(response.memories_created, 0);
    assert!(response.id_mapping.is_empty());

    // Memories should be unchanged
    let list = service.list_memories(mnemonic::service::ListParams {
        agent_id: Some("agent-diff".to_string()),
        session_id: None, tag: None, after: None, before: None,
        limit: None, offset: None,
    }).await.unwrap();
    assert_eq!(list.total, 2, "no memories should be removed when no clusters form");
}

/// DEDUP-03: Compacting Agent A's memories must not affect Agent B's memories.
#[tokio::test]
async fn test_compact_agent_isolation() {
    use mnemonic::service::CreateMemoryRequest;

    let (compaction, service) = build_test_compaction(false).await;

    // Insert identical memories for Agent A
    service.create_memory(CreateMemoryRequest {
        content: "shared fact about the world".to_string(),
        agent_id: Some("agent-A".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "shared fact about the world".to_string(),
        agent_id: Some("agent-A".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();

    // Insert memory for Agent B
    service.create_memory(CreateMemoryRequest {
        content: "agent B private memory".to_string(),
        agent_id: Some("agent-B".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();

    // Compact Agent A only
    let response = compaction.compact(CompactRequest {
        agent_id: "agent-A".to_string(),
        threshold: Some(0.5),
        max_candidates: None,
        dry_run: Some(false),
    }).await.unwrap();

    assert_eq!(response.clusters_found, 1);

    // Verify Agent B is untouched
    let agent_b = service.list_memories(mnemonic::service::ListParams {
        agent_id: Some("agent-B".to_string()),
        session_id: None, tag: None, after: None, before: None,
        limit: None, offset: None,
    }).await.unwrap();
    assert_eq!(agent_b.total, 1, "Agent B must have exactly 1 memory — untouched by Agent A compaction");
    assert_eq!(agent_b.memories[0].content, "agent B private memory");
}

/// DEDUP-04: max_candidates caps the candidate set and sets truncated=true.
#[tokio::test]
async fn test_compact_max_candidates_truncation() {
    use mnemonic::service::CreateMemoryRequest;

    let (compaction, service) = build_test_compaction(false).await;

    // Insert 5 identical memories
    for i in 0..5 {
        service.create_memory(CreateMemoryRequest {
            content: "repeated fact for truncation test".to_string(),
            agent_id: Some("agent-trunc".to_string()),
            session_id: None,
            tags: Some(vec![format!("tag-{}", i)]),
        }).await.unwrap();
    }

    // Compact with max_candidates=3 (should truncate)
    let response = compaction.compact(CompactRequest {
        agent_id: "agent-trunc".to_string(),
        threshold: Some(0.5),
        max_candidates: Some(3),
        dry_run: Some(true),
    }).await.unwrap();

    assert!(response.truncated, "should be truncated when 5 memories exceed max_candidates=3");
    // Clusters should form from only the 3 most recent candidates
    assert!(response.clusters_found >= 1, "should still find clusters within the 3 candidates");
}

/// DEDUP-02: When LLM is configured (MockSummarizer), merged content comes from summarization engine.
#[tokio::test]
async fn test_compact_with_mock_summarizer() {
    use mnemonic::service::CreateMemoryRequest;

    let (compaction, service) = build_test_compaction(true).await;

    // Insert 2 identical-content memories
    service.create_memory(CreateMemoryRequest {
        content: "the sky is blue".to_string(),
        agent_id: Some("agent-llm".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "the sky is blue".to_string(),
        agent_id: Some("agent-llm".to_string()),
        session_id: None, tags: None,
    }).await.unwrap();

    let response = compaction.compact(CompactRequest {
        agent_id: "agent-llm".to_string(),
        threshold: Some(0.5),
        max_candidates: None,
        dry_run: Some(false),
    }).await.unwrap();

    assert_eq!(response.clusters_found, 1);
    assert_eq!(response.memories_created, 1);

    // Verify merged memory content came from MockSummarizer
    let list = service.list_memories(mnemonic::service::ListParams {
        agent_id: Some("agent-llm".to_string()),
        session_id: None, tag: None, after: None, before: None,
        limit: None, offset: None,
    }).await.unwrap();
    assert_eq!(list.total, 1);
    assert!(
        list.memories[0].content.starts_with("MOCK_SUMMARY:"),
        "merged content should come from MockSummarizer, got: {}",
        list.memories[0].content
    );
}

// ────────────────────────────────────────────────────────────────────────────
// HTTP-layer compaction tests (Phase 9)
// ────────────────────────────────────────────────────────────────────────────

/// Creates AppState with CompactionService for HTTP-layer compaction tests.
/// Returns (AppState, MemoryService) so tests can seed data and route HTTP requests.
async fn build_test_compact_state() -> (AppState, Arc<mnemonic::service::MemoryService>) {
    setup();
    let config = test_config();
    let conn = mnemonic::db::open(&config).await.unwrap();
    let db = Arc::new(conn);
    let embedding: Arc<dyn mnemonic::embedding::EmbeddingEngine> = Arc::new(MockEmbeddingEngine);
    let service = Arc::new(mnemonic::service::MemoryService::new(
        db.clone(), embedding.clone(), "mock-model".to_string(),
    ));
    let compaction = Arc::new(CompactionService::new(
        db.clone(), embedding.clone(), None, "mock-model".to_string(),
    ));
    let key_service = Arc::new(mnemonic::auth::KeyService::new(db.clone()));
    let state = AppState {
        service: service.clone(),
        compaction,
        key_service,
    };
    (state, service)
}

/// API-01, API-03, API-04: POST /memories/compact returns 200 with run_id, clusters_found,
/// memories_merged, memories_created, id_mapping, and truncated fields.
#[tokio::test]
async fn test_compact_http_basic() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_compact_state().await;

    // Seed 2 identical-content memories (MockEmbeddingEngine produces same vector -> cosine sim = 1.0)
    let m1 = service.create_memory(CreateMemoryRequest {
        content: "the cat sat on the mat".to_string(),
        agent_id: Some("agent-http".to_string()),
        session_id: None,
        tags: Some(vec!["animal".to_string()]),
    }).await.unwrap();
    let m2 = service.create_memory(CreateMemoryRequest {
        content: "the cat sat on the mat".to_string(),
        agent_id: Some("agent-http".to_string()),
        session_id: None,
        tags: Some(vec!["pet".to_string()]),
    }).await.unwrap();

    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "agent-http",
            "threshold": 0.5
        })))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;

    // API-01: run_id present
    assert!(json["run_id"].is_string(), "response must have string run_id");
    // API-03: stats present
    assert_eq!(json["clusters_found"], 1);
    assert_eq!(json["memories_merged"], 2);
    assert_eq!(json["memories_created"], 1);
    // API-04: id_mapping present with source_ids and new_id
    let mapping = &json["id_mapping"][0];
    let source_ids: Vec<String> = mapping["source_ids"].as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(source_ids.contains(&m1.id), "source_ids must contain m1");
    assert!(source_ids.contains(&m2.id), "source_ids must contain m2");
    assert!(mapping["new_id"].is_string(), "new_id must be present in non-dry-run");
    // truncated field present
    assert!(json["truncated"].is_boolean(), "response must have boolean truncated field");
}

/// API-02: POST /memories/compact with dry_run=true returns 200 with cluster preview,
/// and GET /memories confirms no data was modified.
#[tokio::test]
async fn test_compact_http_dry_run() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_compact_state().await;

    // Seed 2 identical-content memories
    service.create_memory(CreateMemoryRequest {
        content: "identical content for http dry run".to_string(),
        agent_id: Some("agent-dry-http".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "identical content for http dry run".to_string(),
        agent_id: Some("agent-dry-http".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // POST compact with dry_run=true
    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "agent-dry-http",
            "threshold": 0.5,
            "dry_run": true
        })))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["clusters_found"], 1);
    assert_eq!(json["memories_merged"], 2);
    assert_eq!(json["memories_created"], 0, "dry_run must not create memories");
    // new_id should be null in dry_run
    assert!(json["id_mapping"][0]["new_id"].is_null(), "new_id must be null in dry_run");

    // Verify DB unchanged via GET /memories
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::get("/memories?agent_id=agent-dry-http")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 2, "dry_run must not modify memory count");
}

/// API-01: Compacting Agent A's memories via HTTP leaves Agent B's memories untouched.
#[tokio::test]
async fn test_compact_http_agent_isolation() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, service) = build_test_compact_state().await;

    // Seed identical memories for Agent A
    service.create_memory(CreateMemoryRequest {
        content: "shared fact via http test".to_string(),
        agent_id: Some("http-agent-A".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();
    service.create_memory(CreateMemoryRequest {
        content: "shared fact via http test".to_string(),
        agent_id: Some("http-agent-A".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // Seed memory for Agent B
    service.create_memory(CreateMemoryRequest {
        content: "agent B private http memory".to_string(),
        agent_id: Some("http-agent-B".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // Compact Agent A via HTTP
    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "http-agent-A",
            "threshold": 0.5
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["clusters_found"], 1);

    // Verify Agent B is untouched via GET /memories
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::get("/memories?agent_id=http-agent-B")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["total"], 1, "Agent B must have exactly 1 memory — untouched by Agent A compaction");
    assert_eq!(json["memories"][0]["content"], "agent B private http memory");
}

// ────────────────────────────────────────────────────────────────────────────
// Auth Middleware Integration Tests (Phase 12)
// ────────────────────────────────────────────────────────────────────────────

/// Creates a test app with one active API key.
/// Returns (Router, raw_token) for auth tests that need a valid token.
async fn build_auth_app() -> (axum::Router, String) {
    let (state, _) = build_test_state().await;
    let (_key, raw_token) = state
        .key_service
        .create("test-key".to_string(), None)
        .await
        .unwrap();
    (build_router(state), raw_token)
}

/// AUTH-01: Valid Bearer token allows the request through.
#[tokio::test]
async fn test_auth_valid_token_allows() {
    let (app, token) = build_auth_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// AUTH-02: Invalid token returns 401.
#[tokio::test]
async fn test_auth_invalid_token_rejects() {
    let (app, _valid_token) = build_auth_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", "Bearer mnk_0000000000000000000000000000000000000000000000000000000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let json = response_json(response).await;
    assert_eq!(json["error"], "unauthorized");
}

/// AUTH-02: Revoked token returns 401 (while another active key keeps auth mode active).
#[tokio::test]
async fn test_auth_revoked_token_rejects() {
    let (state, _) = build_test_state().await;
    // Create two keys: revoke the first, keep the second active so auth mode stays on.
    let (api_key, raw_token) = state
        .key_service
        .create("revoke-test".to_string(), None)
        .await
        .unwrap();
    let (_active_key, _active_token) = state
        .key_service
        .create("active-key".to_string(), None)
        .await
        .unwrap();
    state.key_service.revoke(&api_key.id).await.unwrap();
    let app = build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", format!("Bearer {}", raw_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// AUTH-03: Open mode (zero keys) allows all requests without auth header.
#[tokio::test]
async fn test_auth_open_mode_allows() {
    // Fresh DB with zero keys — open mode
    let app = build_test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// AUTH-05: GET /health returns 200 without auth header even when auth is active.
#[tokio::test]
async fn test_auth_health_no_token() {
    let (app, _token) = build_auth_app().await;
    // No Authorization header — health should still return 200
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["status"], "ok");
}

/// Malformed Authorization header (not "Bearer <token>" format) returns 400.
#[tokio::test]
async fn test_auth_malformed_header_400() {
    let (app, _token) = build_auth_app().await;
    // "Token" instead of "Bearer" — malformed
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", "Token some-value")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert!(json["error"].as_str().unwrap().contains("Bearer"), "error message should mention Bearer format");
}

/// Validation: empty agent_id returns 400, threshold out of range returns 400,
/// max_candidates=0 returns 400.
#[tokio::test]
async fn test_compact_http_validation() {
    let (state, _) = build_test_compact_state().await;

    // Empty agent_id -> 400
    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "",
            "threshold": 0.5
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert_eq!(json["error"], "agent_id must not be empty");

    // Whitespace-only agent_id -> 400
    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "   ",
            "threshold": 0.5
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert_eq!(json["error"], "agent_id must not be empty");

    // Threshold > 1.0 -> 400
    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "agent-1",
            "threshold": 1.5
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert_eq!(json["error"], "threshold must be between 0.0 and 1.0");

    // Threshold < 0.0 -> 400
    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "agent-1",
            "threshold": -0.1
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert_eq!(json["error"], "threshold must be between 0.0 and 1.0");

    // max_candidates = 0 -> 400
    let app = build_router(state.clone());
    let response = app
        .oneshot(json_request("POST", "/memories/compact", serde_json::json!({
            "agent_id": "agent-1",
            "max_candidates": 0
        })))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = response_json(response).await;
    assert_eq!(json["error"], "max_candidates must be greater than 0");
}

// ────────────────────────────────────────────────────────────────────────────
// Scope Enforcement & Key Endpoint Tests (Phase 13)
// ────────────────────────────────────────────────────────────────────────────

/// Creates a test app with a scoped key (agent_id = "agent-A").
/// Returns (AppState, Router, raw_token).
async fn build_scoped_auth_app() -> (AppState, axum::Router, String) {
    let (state, _) = build_test_state().await;
    let (_key, raw_token) = state
        .key_service
        .create("scoped-key".to_string(), Some("agent-A".to_string()))
        .await
        .unwrap();
    let app = build_router(state.clone());
    (state, app, raw_token)
}

/// AUTH-04-a: Scoped key for agent-A + body agent_id agent-B returns 403.
#[tokio::test]
async fn test_scope_mismatch_returns_403() {
    let (_state, app, token) = build_scoped_auth_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/memories")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&serde_json::json!({
                    "content": "some memory",
                    "agent_id": "agent-B"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let json = response_json(response).await;
    assert_eq!(json["error"], "forbidden");
    let detail = json["detail"].as_str().unwrap_or("");
    assert!(detail.contains("agent-A"), "detail must mention agent-A, got: {}", detail);
    assert!(detail.contains("agent-B"), "detail must mention agent-B, got: {}", detail);
}

/// AUTH-04-b: Scoped key for agent-A + no agent_id in body -> memory created with agent_id = "agent-A".
#[tokio::test]
async fn test_scope_forces_agent_id() {
    let (state, _, token) = build_scoped_auth_app().await;

    // POST /memories with no agent_id
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/memories")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&serde_json::json!({
                    "content": "forced scope memory"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // GET /memories — verify agent_id is "agent-A"
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let memories = json["memories"].as_array().unwrap();
    assert!(!memories.is_empty(), "should have at least 1 memory");
    assert_eq!(memories[0]["agent_id"], "agent-A", "forced scope should set agent_id to agent-A");
}

/// AUTH-04-c: Wildcard key + body agent_id "any-agent" -> 201 success.
#[tokio::test]
async fn test_wildcard_key_passes_through() {
    // build_auth_app creates a wildcard key (agent_id = None)
    let (app, token) = build_auth_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/memories")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&serde_json::json!({
                    "content": "wildcard memory",
                    "agent_id": "any-agent"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = response_json(response).await;
    assert_eq!(json["agent_id"], "any-agent");
}

/// AUTH-04-d: Scoped key for agent-A + DELETE memory owned by agent-B -> 403.
#[tokio::test]
async fn test_scoped_delete_wrong_owner_403() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, _) = build_test_state().await;

    // Create a memory owned by agent-B directly via service (bypassing auth)
    let memory_b = state.service.create_memory(CreateMemoryRequest {
        content: "agent-B private memory".to_string(),
        agent_id: Some("agent-B".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // Create a scoped key for agent-A
    let (_key, token_a) = state
        .key_service
        .create("scoped-a".to_string(), Some("agent-A".to_string()))
        .await
        .unwrap();

    // Attempt DELETE as agent-A -> should get 403
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/memories/{}", memory_b.id))
                .header("authorization", format!("Bearer {}", token_a))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let json = response_json(response).await;
    assert_eq!(json["error"], "forbidden");
}

/// AUTH-04-e: Scoped key for agent-A + DELETE memory owned by agent-A -> 200.
#[tokio::test]
async fn test_scoped_delete_own_memory_ok() {
    use mnemonic::service::CreateMemoryRequest;

    let (state, _) = build_test_state().await;

    // Create a scoped key for agent-A
    let (_key, token_a) = state
        .key_service
        .create("scoped-a-ok".to_string(), Some("agent-A".to_string()))
        .await
        .unwrap();

    // Create a memory owned by agent-A directly via service
    let memory_a = state.service.create_memory(CreateMemoryRequest {
        content: "agent-A memory to delete".to_string(),
        agent_id: Some("agent-A".to_string()),
        session_id: None,
        tags: None,
    }).await.unwrap();

    // DELETE as agent-A -> should succeed with 200
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/memories/{}", memory_a.id))
                .header("authorization", format!("Bearer {}", token_a))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["id"], memory_a.id);
}

/// KEY-endpoint-a: POST /keys in open mode returns 201 with raw_token and key metadata.
#[tokio::test]
async fn test_post_keys_creates_key() {
    // In open mode (zero keys), no auth needed
    let app = build_test_app().await;
    let response = app
        .oneshot(json_request("POST", "/keys", serde_json::json!({"name": "my-key"})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = response_json(response).await;

    // raw_token must be present and start with "mnk_"
    let raw_token = json["raw_token"].as_str().expect("raw_token must be a string");
    assert!(raw_token.starts_with("mnk_"), "raw_token must start with mnk_, got: {}", raw_token);

    // key object must have required fields
    let key = &json["key"];
    assert!(key["id"].is_string(), "key.id must be a string");
    assert_eq!(key["name"], "my-key");
    assert!(key["display_id"].is_string(), "key.display_id must be a string");
    assert!(key["created_at"].is_string(), "key.created_at must be a string");
}

/// KEY-endpoint-b: GET /keys returns key metadata array; no raw_token or hashed_key fields.
#[tokio::test]
async fn test_get_keys_no_raw_token() {
    let (state, _) = build_test_state().await;

    // Create a key via service directly
    let (_key, first_token) = state
        .key_service
        .create("first-key".to_string(), None)
        .await
        .unwrap();

    // Create a second key via HTTP (requires auth from first key)
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", first_token))
                .body(Body::from(serde_json::to_string(&serde_json::json!({"name": "second-key"})).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // GET /keys with auth
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/keys")
                .header("authorization", format!("Bearer {}", first_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    let keys = json["keys"].as_array().expect("response must have keys array");
    assert!(!keys.is_empty(), "keys array must not be empty");

    // Verify no element has raw_token or hashed_key
    for key in keys {
        assert!(key["raw_token"].is_null(), "raw_token must not appear in GET /keys response");
        assert!(key["hashed_key"].is_null(), "hashed_key must not appear in GET /keys response");
        assert!(key["id"].is_string(), "each key must have an id");
        assert!(key["name"].is_string(), "each key must have a name");
        assert!(key["display_id"].is_string(), "each key must have a display_id");
        assert!(key["created_at"].is_string(), "each key must have a created_at");
    }
}

/// KEY-endpoint-c: DELETE /keys/:id revokes key; subsequent requests with that key return 401.
#[tokio::test]
async fn test_delete_key_revokes_access() {
    let (state, _) = build_test_state().await;

    // Create first key (will be the "admin" key used to make requests)
    let (key1, token1) = state
        .key_service
        .create("admin-key".to_string(), None)
        .await
        .unwrap();

    // Create second key to be revoked (via HTTP with first key auth)
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/keys")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token1))
                .body(Body::from(serde_json::to_string(&serde_json::json!({"name": "to-revoke"})).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let create_json = response_json(response).await;
    let token2 = create_json["raw_token"].as_str().unwrap().to_string();
    let key2_id = create_json["key"]["id"].as_str().unwrap().to_string();

    // Verify token2 works before revocation
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "token2 should work before revocation");

    // DELETE /keys/:id using token1 to revoke key2
    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/keys/{}", key2_id))
                .header("authorization", format!("Bearer {}", token1))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json["revoked"], true);

    // Also revoke key1 to make room — but keep key1 active so auth mode stays on
    // token2 is now revoked, verify it returns 401
    // (key1 is still active, so auth mode remains on)
    let _ = key1; // keep key1 alive

    let app = build_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/memories")
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "revoked token2 should return 401");
}
