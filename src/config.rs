use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

/// Configuration for the Mnemonic service.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub port: u16,
    pub db_path: String,
    pub embedding_provider: String,
    pub openai_api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            db_path: "./mnemonic.db".to_string(),
            embedding_provider: "local".to_string(),
            openai_api_key: None,
        }
    }
}

/// Validates business-rule constraints that cannot be expressed in the type system.
/// Call after load_config() succeeds, before any I/O.
pub fn validate_config(config: &Config) -> anyhow::Result<()> {
    match config.embedding_provider.as_str() {
        "local" => Ok(()),
        "openai" => {
            if config.openai_api_key.is_none() {
                anyhow::bail!(
                    "embedding_provider is \"openai\" but MNEMONIC_OPENAI_API_KEY is not set"
                );
            }
            Ok(())
        }
        other => {
            anyhow::bail!(
                "unknown embedding_provider {:?}: expected \"local\" or \"openai\"",
                other
            );
        }
    }
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
            assert_eq!(config.db_path, "./mnemonic.db");
            assert_eq!(config.embedding_provider, "local");
            assert!(config.openai_api_key.is_none());
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
}
