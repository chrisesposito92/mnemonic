use mnemonic::embedding::{EmbeddingEngine, LocalEngine};
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
    }
}

/// Verifies that the memories table exists after db::open() and contains all 8 required columns.
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

    // Verify all 8 columns are present
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
    ];

    for col in &expected_columns {
        assert!(
            column_names.iter().any(|c| c == col),
            "memories table should have column '{}'",
            col
        );
    }

    assert_eq!(column_names.len(), 8, "memories table should have exactly 8 columns");
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
    let state = AppState {
        db,
        config: Arc::new(config),
        embedding,
        service: service.clone(),
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
