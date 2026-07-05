//! NovaX Storage
//!
//! Unified storage layer supporting PostgreSQL, SQLite, and MySQL.
//! v0.1 provides connection pooling and basic CRUD operations.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod memory;

/// Backend kind identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Postgres,
    Sqlite,
    Mysql,
    Memory,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Sqlite => "sqlite",
            Self::Mysql => "mysql",
            Self::Memory => "memory",
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub backend: BackendKind,
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: BackendKind::Memory,
            url: "memory://".to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout_seconds: 5,
            idle_timeout_seconds: 600,
        }
    }
}

/// Generic key-value storage trait
#[async_trait]
pub trait Storage: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    async fn set(&self, key: &str, value: Vec<u8>) -> Result<(), StorageError>;
    async fn set_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<(), StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;
    async fn health(&self) -> Result<HealthStatus, StorageError>;
    fn backend(&self) -> BackendKind;
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub backend: String,
    pub message: String,
}

/// Storage error types
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("backend error: {0}")]
    Backend(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("configuration error: {0}")]
    Config(String),
}

/// Create a storage backend from configuration
pub async fn create_storage(config: &StorageConfig) -> Result<Box<dyn Storage>, StorageError> {
    match config.backend {
        BackendKind::Memory => Ok(Box::new(memory::MemoryStorage::new())),
        BackendKind::Sqlite => Err(StorageError::Config(
            "SQLite backend not yet implemented in v0.1 scaffold".to_string(),
        )),
        BackendKind::Postgres => Err(StorageError::Config(
            "PostgreSQL backend not yet implemented in v0.1 scaffold".to_string(),
        )),
        BackendKind::Mysql => Err(StorageError::Config(
            "MySQL backend not yet implemented in v0.1 scaffold".to_string(),
        )),
    }
}
