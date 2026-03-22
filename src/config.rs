use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

/// Configuration for the Mnemonic service.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub port: u16,
    /// Port for the gRPC server. Defaults to 50051. Set via MNEMONIC_GRPC_PORT env var or grpc_port in TOML.
    pub grpc_port: u16,
    pub db_path: String,
    pub embedding_provider: String,
    pub openai_api_key: Option<String>,
    pub llm_provider: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_base_url: Option<String>,
    pub llm_model: Option<String>,
    /// Storage backend to use. Defaults to "sqlite". Valid values: "sqlite", "qdrant", "postgres".
    /// Set via MNEMONIC_STORAGE_PROVIDER env var or storage_provider in TOML config.
    pub storage_provider: String,
    /// URL for Qdrant backend. Required when storage_provider is "qdrant".
    /// Set via MNEMONIC_QDRANT_URL env var or qdrant_url in TOML config.
    pub qdrant_url: Option<String>,
    /// API key for Qdrant backend (optional even when qdrant_url is set).
    /// Set via MNEMONIC_QDRANT_API_KEY env var or qdrant_api_key in TOML config.
    pub qdrant_api_key: Option<String>,
    /// Connection URL for Postgres backend. Required when storage_provider is "postgres".
    /// Set via MNEMONIC_POSTGRES_URL env var or postgres_url in TOML config.
    pub postgres_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            grpc_port: 50051,
            db_path: "./mnemonic.db".to_string(),
            embedding_provider: "local".to_string(),
            openai_api_key: None,
            llm_provider: None,
            llm_api_key: None,
            llm_base_url: None,
            llm_model: None,
            storage_provider: "sqlite".to_string(),
            qdrant_url: None,
            qdrant_api_key: None,
            postgres_url: None,
        }
    }
}

/// Validates business-rule constraints that cannot be expressed in the type system.
/// Call after load_config() succeeds, before any I/O.
pub fn validate_config(config: &Config) -> anyhow::Result<()> {
    // Embedding provider validation
    match config.embedding_provider.as_str() {
        "local" => {}
        "openai" => {
            if config.openai_api_key.is_none() {
                anyhow::bail!(
                    "embedding_provider is \"openai\" but MNEMONIC_OPENAI_API_KEY is not set"
                );
            }
        }
        other => {
            anyhow::bail!(
                "unknown embedding_provider {:?}: expected \"local\" or \"openai\"",
                other
            );
        }
    }

    // LLM validation (independent of embedding validation)
    if let Some(provider) = &config.llm_provider {
        match provider.as_str() {
            "openai" => {
                if config.llm_api_key.is_none() {
                    anyhow::bail!(
                        "llm_provider is \"openai\" but MNEMONIC_LLM_API_KEY is not set"
                    );
                }
            }
            other => {
                anyhow::bail!(
                    "unknown llm_provider {:?}: expected \"openai\"",
                    other
                );
            }
        }
    }

    // Storage provider validation (per D-04 through D-09)
    // Note: validation passes for "qdrant"/"postgres" even when built without the feature flag —
    // the feature-gate error comes at backend construction time (create_backend), not here.
    match config.storage_provider.as_str() {
        "sqlite" => {} // db_path already validated by existing logic
        "qdrant" => {
            if config.qdrant_url.is_none() {
                anyhow::bail!(
                    "storage_provider is \"qdrant\" but MNEMONIC_QDRANT_URL is not set"
                );
            }
        }
        "postgres" => {
            if config.postgres_url.is_none() {
                anyhow::bail!(
                    "storage_provider is \"postgres\" but MNEMONIC_POSTGRES_URL is not set"
                );
            }
        }
        other => {
            anyhow::bail!(
                "unknown storage_provider {:?}: expected \"sqlite\", \"qdrant\", or \"postgres\"",
                other
            );
        }
    }

    Ok(())
}

/// Loads configuration from defaults, optional TOML file, and environment variables.
///
/// Precedence (highest to lowest):
///   1. Environment variables prefixed with `MNEMONIC_`
///   2. TOML file at `MNEMONIC_CONFIG_PATH` (defaults to `mnemonic.toml` in CWD)
///   3. Compiled-in defaults
///
/// A missing TOML file is silently ignored — not an error.
pub fn load_config() -> Result<Config, crate::error::ConfigError> {
    let toml_path = std::env::var("MNEMONIC_CONFIG_PATH")
        .unwrap_or_else(|_| "mnemonic.toml".to_string());

    Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file(&toml_path))
        .merge(Env::prefixed("MNEMONIC_"))
        .extract::<Config>()
        .map_err(|e| crate::error::ConfigError::Load(format!("{}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        figment::Jail::expect_with(|_jail: &mut figment::Jail| {
            let config = load_config().unwrap();
            assert_eq!(config.port, 8080);
            assert_eq!(config.grpc_port, 50051);
            assert_eq!(config.db_path, "./mnemonic.db");
            assert_eq!(config.embedding_provider, "local");
            assert!(config.openai_api_key.is_none());
            assert!(config.llm_provider.is_none());
            assert!(config.llm_api_key.is_none());
            assert!(config.llm_base_url.is_none());
            assert!(config.llm_model.is_none());
            assert_eq!(config.storage_provider, "sqlite");
            assert!(config.qdrant_url.is_none());
            assert!(config.qdrant_api_key.is_none());
            assert!(config.postgres_url.is_none());
            Ok(())
        });
    }

    #[test]
    fn test_config_env_override() {
        figment::Jail::expect_with(|jail: &mut figment::Jail| {
            jail.set_env("MNEMONIC_PORT", "9090");
            let config = load_config().unwrap();
            assert_eq!(config.port, 9090);
            Ok(())
        });
    }

    #[test]
    fn test_grpc_port_env_override() {
        figment::Jail::expect_with(|jail: &mut figment::Jail| {
            jail.set_env("MNEMONIC_GRPC_PORT", "50052");
            let config = load_config().unwrap();
            assert_eq!(config.grpc_port, 50052);
            Ok(())
        });
    }

    #[test]
    fn test_config_toml_override() {
        figment::Jail::expect_with(|jail: &mut figment::Jail| {
            jail.create_file("mnemonic.toml", "port = 7070\n")?;
            let config = load_config().unwrap();
            assert_eq!(config.port, 7070);
            Ok(())
        });
    }

    #[test]
    fn test_config_env_beats_toml() {
        figment::Jail::expect_with(|jail: &mut figment::Jail| {
            jail.set_env("MNEMONIC_PORT", "9090");
            jail.create_file("mnemonic.toml", "port = 7070\n")?;
            let config = load_config().unwrap();
            assert_eq!(config.port, 9090);
            Ok(())
        });
    }

    #[test]
    fn test_config_missing_toml_ok() {
        figment::Jail::expect_with(|_jail: &mut figment::Jail| {
            // No mnemonic.toml created — should return defaults without error
            let config = load_config().unwrap();
            assert_eq!(config.port, 8080);
            assert_eq!(config.db_path, "./mnemonic.db");
            assert_eq!(config.embedding_provider, "local");
            Ok(())
        });
    }

    #[test]
    fn test_validate_config_openai_no_key() {
        let config = Config {
            embedding_provider: "openai".to_string(),
            openai_api_key: None,
            ..Config::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(err.to_string().contains("MNEMONIC_OPENAI_API_KEY"), "error was: {}", err);
    }

    #[test]
    fn test_validate_config_unknown_provider() {
        let config = Config {
            embedding_provider: "postgres".to_string(),
            ..Config::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(err.to_string().contains("unknown embedding_provider"), "error was: {}", err);
    }

    #[test]
    fn test_validate_config_local_ok() {
        let config = Config::default(); // embedding_provider defaults to "local"
        validate_config(&config).unwrap();
    }

    #[test]
    fn test_validate_config_openai_with_key() {
        let config = Config {
            embedding_provider: "openai".to_string(),
            openai_api_key: Some("sk-test".to_string()),
            ..Config::default()
        };
        validate_config(&config).unwrap();
    }

    #[test]
    fn test_validate_config_llm_openai_no_key() {
        let config = Config {
            llm_provider: Some("openai".to_string()),
            llm_api_key: None,
            ..Config::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(err.to_string().contains("MNEMONIC_LLM_API_KEY"), "error was: {}", err);
    }

    #[test]
    fn test_validate_config_llm_openai_with_key() {
        let config = Config {
            llm_provider: Some("openai".to_string()),
            llm_api_key: Some("sk-test".to_string()),
            ..Config::default()
        };
        validate_config(&config).unwrap();
    }

    #[test]
    fn test_validate_config_llm_unknown_provider() {
        let config = Config {
            llm_provider: Some("anthropic".to_string()),
            ..Config::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(err.to_string().contains("unknown llm_provider"), "error was: {}", err);
    }

    #[test]
    fn test_validate_config_no_llm_ok() {
        let config = Config::default(); // llm_provider defaults to None
        validate_config(&config).unwrap();
    }

    // ──────────────────────────────────────────────────────────────────────
    // Storage provider validation tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn test_config_defaults_storage_provider() {
        figment::Jail::expect_with(|_jail: &mut figment::Jail| {
            let config = load_config().unwrap();
            assert_eq!(config.storage_provider, "sqlite");
            assert!(config.qdrant_url.is_none());
            assert!(config.qdrant_api_key.is_none());
            assert!(config.postgres_url.is_none());
            Ok(())
        });
    }

    #[test]
    fn test_validate_config_sqlite_ok() {
        let config = Config::default(); // storage_provider defaults to "sqlite"
        validate_config(&config).unwrap();
    }

    #[test]
    fn test_validate_config_qdrant_no_url() {
        let config = Config {
            storage_provider: "qdrant".to_string(),
            qdrant_url: None,
            ..Config::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("MNEMONIC_QDRANT_URL"),
            "error was: {}",
            err
        );
    }

    #[test]
    fn test_validate_config_qdrant_with_url() {
        let config = Config {
            storage_provider: "qdrant".to_string(),
            qdrant_url: Some("http://localhost:6334".to_string()),
            ..Config::default()
        };
        validate_config(&config).unwrap();
    }

    #[test]
    fn test_validate_config_postgres_no_url() {
        let config = Config {
            storage_provider: "postgres".to_string(),
            postgres_url: None,
            ..Config::default()
        };
        let err = validate_config(&config).unwrap_err();
        assert!(
            err.to_string().contains("MNEMONIC_POSTGRES_URL"),
            "error was: {}",
            err
        );
    }

    #[test]
    fn test_validate_config_postgres_with_url() {
        let config = Config {
            storage_provider: "postgres".to_string(),
            postgres_url: Some("postgres://user:pass@localhost/mnemonic".to_string()),
            ..Config::default()
        };
        validate_config(&config).unwrap();
    }

    #[test]
    fn test_validate_config_unknown_storage_provider() {
        let config = Config {
            storage_provider: "redis".to_string(),
            ..Config::default()
        };
        let err = validate_config(&config).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown storage_provider"), "error was: {}", msg);
        assert!(msg.contains("sqlite"), "error was: {}", msg);
        assert!(msg.contains("qdrant"), "error was: {}", msg);
        assert!(msg.contains("postgres"), "error was: {}", msg);
    }

    #[test]
    fn test_storage_provider_env_override() {
        figment::Jail::expect_with(|jail: &mut figment::Jail| {
            jail.set_env("MNEMONIC_STORAGE_PROVIDER", "qdrant");
            jail.set_env("MNEMONIC_QDRANT_URL", "http://localhost:6334");
            let config = load_config().unwrap();
            assert_eq!(config.storage_provider, "qdrant");
            assert_eq!(config.qdrant_url, Some("http://localhost:6334".to_string()));
            Ok(())
        });
    }

    #[test]
    fn test_storage_provider_toml_override() {
        figment::Jail::expect_with(|jail: &mut figment::Jail| {
            jail.create_file(
                "mnemonic.toml",
                "storage_provider = \"postgres\"\npostgres_url = \"postgres://localhost/db\"\n",
            )?;
            let config = load_config().unwrap();
            assert_eq!(config.storage_provider, "postgres");
            assert_eq!(
                config.postgres_url,
                Some("postgres://localhost/db".to_string())
            );
            Ok(())
        });
    }
}
