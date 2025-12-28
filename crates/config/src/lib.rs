//! Application configuration loaded from TOML and stored globally.
//!
//! This crate exposes a single global configuration value backed by a `OnceLock`
//! so callers can access settings without threading them through call stacks.

use serde::Deserialize;
use std::fs;
use std::sync::OnceLock;

/// Global configuration instance.
static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// Top-level application configuration.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub database_path: String,
    pub migrations_dir: String,
    pub max_users: u32,
}

/// Load configuration from a TOML file and initialize the global config.
///
/// Subsequent calls return an error to prevent accidental reconfiguration.
fn load_config() -> AppConfig {
    let contents = fs::read_to_string("./config.toml")
        .unwrap_or_else(|e| panic!("error, when reading config contents. Error: {e}"));
    toml::from_str::<AppConfig>(&contents)
        .unwrap_or_else(|e| panic!("error, config failed to load. Error: {e}"))
}

/// Access the initialized configuration.
pub fn get_config() -> &'static AppConfig {
    CONFIG.get_or_init(load_config)
}
