use anyhow::Result;

mod compaction;
mod config;
mod db;
mod embedding;
mod error;
mod server;
mod service;
mod summarization;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Register sqlite-vec BEFORE any Connection::open
    db::register_sqlite_vec();

    // 2. Init tracing
    server::init_tracing();

    // 3. Load config (defaults -> TOML -> env vars)
    let config = config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;
    config::validate_config(&config)?;

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
        match config.embedding_provider.as_str() {
            "local" => {
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
            }
            "openai" => {
                let api_key = config.openai_api_key.as_ref().unwrap(); // safe: validate_config passed
                tracing::info!(
                    provider = "openai",
                    model = "text-embedding-3-small",
                    dimensions = 384,
                    "embedding engine ready"
                );
                std::sync::Arc::new(embedding::OpenAiEngine::new(api_key.clone()))
            }
            _ => unreachable!(), // validate_config rejects unknown providers
        };

    // 5b. Initialize LLM summarization engine (optional)
    let llm_engine: Option<std::sync::Arc<dyn summarization::SummarizationEngine>> =
        match config.llm_provider.as_deref() {
            Some("openai") => {
                let api_key = config.llm_api_key.as_ref().unwrap(); // safe: validate_config passed
                let base_url = config.llm_base_url.clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
                let model = config.llm_model.clone()
                    .unwrap_or_else(|| "gpt-4o-mini".to_string());
                tracing::info!(
                    provider = "openai",
                    model = %model,
                    "LLM summarization engine ready"
                );
                Some(std::sync::Arc::new(
                    summarization::OpenAiSummarizer::new(api_key.clone(), base_url, model)
                ))
            }
            None => None,
            _ => unreachable!(), // validate_config rejects unknown providers
        };

    // 6. Build MemoryService
    let embedding_model = match config.embedding_provider.as_str() {
        "openai" => "text-embedding-3-small".to_string(),
        _        => "all-MiniLM-L6-v2".to_string(),
    };

    let db_arc = std::sync::Arc::new(conn);
    let service = std::sync::Arc::new(
        service::MemoryService::new(
            db_arc.clone(),
            embedding.clone(),
            embedding_model.clone(),
        )
    );

    // 6b. Build CompactionService (consumed by HTTP handler in Phase 9)
    let _compaction = std::sync::Arc::new(
        compaction::CompactionService::new(
            db_arc.clone(),
            embedding.clone(),
            llm_engine,
            embedding_model.clone(),
        )
    );

    // 7. Start axum server
    let state = server::AppState {
        service,
    };
    server::serve(&config, state).await?;

    Ok(())
}
