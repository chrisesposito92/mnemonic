/// Top-level error enum for the Mnemonic service.
#[derive(Debug, thiserror::Error)]
pub enum MnemonicError {
    #[error("database error: {0}")]
    Db(#[from] DbError),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),

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
