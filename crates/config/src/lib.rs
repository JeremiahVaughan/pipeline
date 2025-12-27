//! Application configuration loaded from TOML and stored globally.
//!
//! This crate exposes a single global configuration value backed by a `OnceLock`
//! so callers can access settings without threading them through call stacks.

use serde::Deserialize;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// Global configuration instance.
static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// Top-level application configuration.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub database_path: String,
}

/// Errors that can occur during configuration loading.
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    ParseToml(toml::de::Error),
    AlreadyInitialized,
    NotInitialized,
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::ParseToml(err) => write!(f, "TOML parse error: {err}"),
            Self::AlreadyInitialized => write!(f, "configuration already initialized"),
            Self::NotInitialized => write!(f, "configuration not initialized"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        Self::ParseToml(err)
    }
}

/// Load configuration from a TOML file and initialize the global config.
///
/// Subsequent calls return an error to prevent accidental reconfiguration.
pub fn load_config(path: impl AsRef<Path>) -> Result<&'static AppConfig, ConfigError> {
    let contents = fs::read_to_string(path)?;
    let parsed: AppConfig = toml::from_str(&contents)?;
    CONFIG
        .set(parsed)
        .map_err(|_| ConfigError::AlreadyInitialized)?;
    CONFIG.get().ok_or(ConfigError::NotInitialized)
}

/// Access the initialized configuration.
pub fn get_config() -> Result<&'static AppConfig, ConfigError> {
    CONFIG.get().ok_or(ConfigError::NotInitialized)
}

#[cfg(test)]
mod tests {
    use super::{ConfigError, get_config, load_config};
    use std::fs;

    #[test]
    fn loads_config_and_stores_globally_and_rejects_reinit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "database_path = \"data/app.db\"").expect("write");

        let loaded = load_config(&config_path).expect("load config");
        assert_eq!(
            loaded,
            get_config().expect("should retrieve config"),
            "get_config should return the same reference"
        );
        assert_eq!(loaded.database_path, "data/app.db");

        let second = load_config(&config_path);
        assert!(
            matches!(second, Err(ConfigError::AlreadyInitialized)),
            "re-initialization should fail"
        );
    }
}
