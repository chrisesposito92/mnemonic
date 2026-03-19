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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            db_path: "./mnemonic.db".to_string(),
            embedding_provider: "local".to_string(),
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
}
