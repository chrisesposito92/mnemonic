use anyhow::Result;

mod config;
mod db;
mod embedding;
mod error;
mod server;
mod service;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Register sqlite-vec BEFORE any Connection::open
    db::register_sqlite_vec();

    // 2. Init tracing
    server::init_tracing();

    // 3. Load config (defaults -> TOML -> env vars)
    let config = config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        port = config.port,
        db_path = %config.db_path,
        embedding_provider = %config.embedding_provider,
        "mnemonic starting"
    );

    // 4. Open DB and apply schema
    let conn = db::open(&config).await
        .map_err(|e| anyhow::anyhow!(e))?;

    tracing::info!("database initialized (WAL mode)");

    // 5. Initialize embedding engine
    let embedding: std::sync::Arc<dyn embedding::EmbeddingEngine> =
        if let Some(ref api_key) = config.openai_api_key {
            tracing::info!(
                provider = "openai",
                model = "text-embedding-3-small",
                dimensions = 384,
                "embedding engine ready"
            );
            std::sync::Arc::new(embedding::OpenAiEngine::new(api_key.clone()))
        } else {
            let start = std::time::Instant::now();
            tracing::info!(
                provider = "local",
                model = "all-MiniLM-L6-v2",
                "loading embedding model..."
            );
            let engine = tokio::task::spawn_blocking(|| {
                embedding::LocalEngine::new()
            })
            .await?
            .map_err(|e| anyhow::anyhow!(e))?;
            tracing::info!(
                elapsed_ms = start.elapsed().as_millis() as u64,
                "embedding model loaded"
            );
            std::sync::Arc::new(engine)
        };

    // 6. Build MemoryService
    let embedding_model = if config.openai_api_key.is_some() {
        "text-embedding-3-small".to_string()
    } else {
        "all-MiniLM-L6-v2".to_string()
    };

    let db_arc = std::sync::Arc::new(conn);
    let service = std::sync::Arc::new(
        service::MemoryService::new(
            db_arc.clone(),
            embedding.clone(),
            embedding_model,
        )
    );

    // 7. Start axum server
    let state = server::AppState {
        db: db_arc,
        config: std::sync::Arc::new(config.clone()),
        embedding,
        service,
    };
    server::serve(&config, state).await?;

    Ok(())
}
