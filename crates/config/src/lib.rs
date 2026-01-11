//! Application configuration loaded from TOML and stored globally.
//!
//! This crate exposes a single global configuration value backed by a `OnceLock`
//! so callers can access settings without threading them through call stacks.

use serde::Deserialize;
use serde::de::{self, Deserializer};
use std::collections::BTreeMap;
use std::fs;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Global configuration instance.
static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// Top-level application configuration.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct AppConfig {

    #[serde(skip_deserializing)]
    pub app_version: String,

    pub environment: String,
    pub database_path: String,
    pub migrations_dir: String,
    pub max_users: usize,
    pub repos: BTreeMap<String, RepoCloneConfig>,
    pub nodes: BTreeMap<String, NodeConfig>,
    pub ci: CiConfig,
    pub environments: BTreeMap<String, EnvironmentConfig>,
    pub services: BTreeMap<String, ServiceConfig>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct RepoCloneConfig {
    pub vcs: String,
    pub clone_url: String,
    pub dir: String,
    pub db_file: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct NodeConfig {
    pub host_name: String,
    pub user: String,
    #[serde(deserialize_with = "deserialize_port")]
    pub port: usize,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct EnvironmentConfig {
    pub nodes: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct CiConfig {
    pub nodes: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct ServiceConfig {
    pub create_workspace: String,
    pub build_workspace: String,
    pub deploy_workspace: String,
    pub deploy_as_root: bool,
    #[serde(flatten)]
    pub environments: BTreeMap<String, Vec<ServiceEnvironmentConfig>>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct ServiceEnvironmentConfig {
    pub nodes: Vec<String>,
}

/// Load configuration from a TOML file and initialize the global config.
///
/// Subsequent calls return an error to prevent accidental reconfiguration.
fn load_config() -> AppConfig {
    let contents = fs::read_to_string("./config.toml")
        .unwrap_or_else(|e| panic!("error, when reading config contents. Error: {e}"));
    let mut config = toml::from_str::<AppConfig>(&contents)
        .unwrap_or_else(|e| panic!("error, config failed to load. Error: {e}"));
    validate_config(&config)
        .unwrap_or_else(|e| panic!("error, config failed validation. Error: {e}"));
    if config.environment == "development" {
        let current_time = SystemTime::now();
        let duration_since_epoch = current_time
            .duration_since(UNIX_EPOCH)
            .expect("SystemTime set to a time before UNIX EPOCH!");
        let epoch_seconds = duration_since_epoch.as_secs();
        config.app_version = epoch_seconds.to_string();
    } else {
        // todo need to figure out how to get git commit as the id
    }
    config
}

/// Access the initialized configuration.
pub fn get_config() -> &'static AppConfig {
    CONFIG.get_or_init(load_config)
}

fn validate_config(config: &AppConfig) -> Result<(), String> {
    if config.database_path.is_empty() {
        return Err("database_path is required".to_string());
    }
    if config.migrations_dir.is_empty() {
        return Err("migrations_dir is required".to_string());
    }
    if config.max_users == 0 {
        return Err("max_users must be greater than zero".to_string());
    }

    if config.repos.is_empty() {
        return Err("no repos provided".to_string());
    }

    for (repo_name, repo_cfg) in &config.repos {
        if repo_cfg.vcs.is_empty() {
            return Err(format!("repo {repo_name} requires a vcs"));
        }
        if repo_cfg.clone_url.is_empty() {
            return Err(format!("repo {repo_name} requires a clone_url"));
        }
        if repo_cfg.dir.is_empty() {
            return Err(format!("repo {repo_name} requires a dir"));
        }
        if repo_cfg.vcs == "fossil" && repo_cfg.db_file.is_none() {
            return Err(format!("repo {repo_name} requires a db_file"));
        }
    }

    if config.nodes.is_empty() {
        return Err("no nodes provided".to_string());
    }

    for (node_name, node_cfg) in &config.nodes {
        if node_cfg.host_name.is_empty() {
            return Err(format!("node {} requires a host_name", node_name));
        }
        if node_cfg.user.is_empty() {
            return Err(format!("node {} requires a user", node_name));
        }
        if node_cfg.port == 0 {
            return Err(format!("node {} requires a port", node_name));
        }
    }

    if config.ci.nodes.is_empty() {
        return Err("no ci nodes defined".to_string());
    }

    if config.environments.is_empty() {
        return Err("no environments defined".to_string());
    }

    for (env_name, env_cfg) in &config.environments {
        if env_cfg.nodes.is_empty() {
            return Err(format!("environment '{env_name}' requires at least one node"));
        }
        for node in &env_cfg.nodes {
            if !config.nodes.contains_key(node) {
                return Err(format!(
                    "environment '{env_name}' references unknown node '{node}'"
                ));
            }
        }
    }

    if config.services.is_empty() {
        return Err(format!("services requires at least one entry"));
    }
    for (service_name, service_cfg) in &config.services {
        if service_cfg.create_workspace.is_empty() {
            return Err(format!(
                "service '{service_name}' requires create_workspace"
            ));
        }
        if service_cfg.build_workspace.is_empty() {
            return Err(format!(
                "service '{service_name}' requires build_workspace"
            ));
        }
        if service_cfg.deploy_workspace.is_empty() {
            return Err(format!(
                "service '{service_name}' requires deploy_workspace"
            ));
        }
        if service_cfg.environments.is_empty() {
            return Err(format!(
                "service '{service_name}' requires environments"
            ));
        }
        for (env_name, env_cfgs) in &service_cfg.environments {
            config.environments.get(env_name).ok_or_else(|| {
                format!(
                    "service '{service_name}' references unknown environment '{env_name}'"
                )
            })?;
            if env_cfgs.is_empty() {
                return Err(format!(
                    "service '{service_name}' environment '{env_name}' requires at least one entry"
                ));
            }
            for (env_idx, env_cfg) in env_cfgs.iter().enumerate() {
                if env_cfg.nodes.is_empty() {
                    return Err(format!(
                        "service '{service_name}' environment '{env_name}'[{env_idx}] requires at least one node"
                    ));
                }
                for node in &env_cfg.nodes {
                    if !config.nodes.contains_key(node) {
                        return Err(format!(
                            "service '{service_name}' environment '{env_name}'[{env_idx}] references unknown node '{node}'",
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

fn deserialize_port<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    struct PortVisitor;

    impl<'de> de::Visitor<'de> for PortVisitor {
        type Value = usize;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a port as an integer")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            usize::try_from(value).map_err(|_| E::custom("port is too large"))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value < 0 {
                return Err(E::custom("port must be non-negative"));
            }
            usize::try_from(value).map_err(|_| E::custom("port is too large"))
        }
    }

    deserializer.deserialize_any(PortVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses_and_validates() {
        let contents = include_str!("../../../config/example.toml");
        let config = toml::from_str::<AppConfig>(contents)
            .unwrap_or_else(|e| panic!("failed to parse example config: {e}"));
        validate_config(&config)
            .unwrap_or_else(|e| panic!("example config failed validation: {e}"));
    }
}
