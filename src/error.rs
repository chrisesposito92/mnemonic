/// Top-level error enum for the Mnemonic service.
#[derive(Debug, thiserror::Error)]
pub enum MnemonicError {
    #[error("database error: {0}")]
    Db(#[from] DbError),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    #[error("llm error: {0}")]
    Llm(#[from] LlmError),
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

/// Errors originating from LLM operations.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("LLM API call failed: {0}")]
    ApiCall(String),

    #[error("LLM request timed out")]
    Timeout,

    #[error("LLM response could not be parsed: {0}")]
    ParseError(String),
}

/// API-layer errors with HTTP status code mapping.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("internal error: {0}")]
    Internal(#[from] MnemonicError),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            ApiError::BadRequest(msg) => (
                axum::http::StatusCode::BAD_REQUEST,
                serde_json::json!({"error": msg}),
            ),
            ApiError::NotFound => (
                axum::http::StatusCode::NOT_FOUND,
                serde_json::json!({"error": "not found"}),
            ),
            ApiError::Unauthorized(_) => (
                axum::http::StatusCode::UNAUTHORIZED,
                serde_json::json!({
                    "error": "unauthorized",
                    "auth_mode": "active",
                    "hint": "Provide Authorization: Bearer mnk_..."
                }),
            ),
            ApiError::Forbidden(detail) => (
                axum::http::StatusCode::FORBIDDEN,
                serde_json::json!({
                    "error": "forbidden",
                    "detail": detail
                }),
            ),
            ApiError::Internal(e) => {
                tracing::error!(error = %e, "internal server error");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    serde_json::json!({"error": "internal server error"}),
                )
            }
        };
        (status, axum::Json(body)).into_response()
    }
}

impl From<EmbeddingError> for ApiError {
    fn from(e: EmbeddingError) -> Self {
        match e {
            EmbeddingError::EmptyInput => ApiError::BadRequest("content must not be empty".to_string()),
            other => ApiError::Internal(MnemonicError::Embedding(other)),
        }
    }
}

impl From<tokio_rusqlite::Error> for ApiError {
    fn from(e: tokio_rusqlite::Error) -> Self {
        ApiError::Internal(MnemonicError::Db(DbError::Query(e.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    /// AUTH-04: ApiError::Forbidden returns HTTP 403 with {"error":"forbidden","detail":"..."}.
    #[tokio::test]
    async fn test_forbidden_variant_returns_403_with_correct_body() {
        let err = ApiError::Forbidden("key scoped to agent-A cannot access agent-B".to_string());
        let response = err.into_response();

        assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);

        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(json["error"], "forbidden", "error field must be literal string 'forbidden'");
        assert_eq!(
            json["detail"],
            "key scoped to agent-A cannot access agent-B",
            "detail field must contain the message passed to Forbidden"
        );
    }
}
