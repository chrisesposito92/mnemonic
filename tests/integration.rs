use mnemonic::embedding::{EmbeddingEngine, LocalEngine};
use std::sync::{Arc, Once, OnceLock};

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
