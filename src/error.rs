/// Top-level error enum for the Mnemonic service.
#[derive(Debug, thiserror::Error)]
pub enum MnemonicError {
    #[error("database error: {0}")]
    Db(#[from] DbError),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    #[error("server error: {0}")]
    Server(String),
}

/// Errors originating from database operations.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("failed to open database: {0}")]
    Open(String),

    #[error("schema initialization failed: {0}")]
    Schema(String),

    #[error("query failed: {0}")]
    Query(String),
}

impl From<tokio_rusqlite::Error> for DbError {
    fn from(e: tokio_rusqlite::Error) -> Self {
        DbError::Query(format!("{}", e))
    }
}

/// Errors originating from configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to load configuration: {0}")]
    Load(String),

    #[error("invalid configuration: {0}")]
    Invalid(String),
}

/// Errors originating from embedding operations.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("failed to load embedding model: {0}")]
    ModelLoad(String),

    #[error("embedding inference failed: {0}")]
    Inference(String),

    #[error("embedding API call failed: {0}")]
    ApiCall(String),

    #[error("empty input text — cannot embed empty string")]
    EmptyInput,
}

impl From<candle_core::Error> for EmbeddingError {
    fn from(e: candle_core::Error) -> Self {
        EmbeddingError::Inference(format!("{}", e))
    }
}
