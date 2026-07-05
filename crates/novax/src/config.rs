//! NovaX configuration

use std::net::SocketAddr;

use novax_router::RouterConfig;
use novax_storage::StorageConfig;
use serde::{Deserialize, Serialize};

/// Top-level NovaX configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovaXConfig {
    pub app_name: String,
    pub environment: Environment,
    pub server: ServerConfig,
    pub router: RouterConfig,
    pub storage: StorageConfig,
    pub log_level: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Production,
    Test,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Development
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub workers: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:3000".parse().unwrap(),
            workers: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        }
    }
}

impl Default for NovaXConfig {
    fn default() -> Self {
        Self {
            app_name: "NovaX App".to_string(),
            environment: Environment::default(),
            server: ServerConfig::default(),
            router: RouterConfig::default(),
            storage: StorageConfig::default(),
            log_level: "info".to_string(),
        }
    }
}

impl NovaXConfig {
    /// Load configuration from a TOML file (if novax.toml exists)
    pub fn load_from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileRead(e.to_string()))?;

        // Simple manual parse for v0.1 — full TOML parse coming in v0.2
        // For now, return default with overrides from env
        let mut config = Self::default();

        // Override from environment variables (12-factor app)
        if let Ok(port) = std::env::var("PORT") {
            if let Ok(port) = port.parse::<u16>() {
                config.server.bind_addr = format!("0.0.0.0:{}", port).parse().unwrap();
            }
        }
        if let Ok(env) = std::env::var("NOVAX_ENV") {
            config.environment = match env.to_lowercase().as_str() {
                "production" | "prod" => Environment::Production,
                "test" => Environment::Test,
                _ => Environment::Development,
            };
        }
        if let Ok(log_level) = std::env::var("RUST_LOG") {
            config.log_level = log_level;
        }
        if let Ok(url) = std::env::var("DATABASE_URL") {
            config.storage.backend = if url.starts_with("postgres://") {
                novax_storage::BackendKind::Postgres
            } else if url.starts_with("mysql://") {
                novax_storage::BackendKind::Mysql
            } else if url.starts_with("sqlite://") || url.ends_with(".db") {
                novax_storage::BackendKind::Sqlite
            } else {
                novax_storage::BackendKind::Memory
            };
            config.storage.url = url;
        }

        let _ = content;  // suppress unused warning
        Ok(config)
    }

    /// Load configuration with defaults from environment
    pub fn from_env() -> Self {
        Self::load_from_file("novax.toml").unwrap_or_default()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("file read error: {0}")]
    FileRead(String),
    #[error("parse error: {0}")]
    Parse(String),
}
