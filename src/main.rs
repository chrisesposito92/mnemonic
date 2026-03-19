use anyhow::Result;

mod config;
mod db;
mod error;
mod server;

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

    // 5. Start axum server
    let state = server::AppState {
        db: std::sync::Arc::new(conn),
        config: std::sync::Arc::new(config.clone()),
    };
    server::serve(&config, state).await?;

    Ok(())
}
