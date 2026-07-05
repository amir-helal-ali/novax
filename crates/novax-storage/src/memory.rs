//! In-memory storage backend (default for development)

use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use parking_lot::RwLock;

use crate::{BackendKind, HealthStatus, Storage, StorageError};

/// In-memory storage implementation
pub struct MemoryStorage {
    data: RwLock<HashMap<String, Entry>>,
}

struct Entry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let data = self.data.read();
        if let Some(entry) = data.get(key) {
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() >= expires_at {
                    return Ok(None);
                }
            }
            Ok(Some(entry.value.clone()))
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>) -> Result<(), StorageError> {
        self.data.write().insert(
            key.to_string(),
            Entry {
                value,
                expires_at: None,
            },
        );
        Ok(())
    }

    async fn set_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<(), StorageError> {
        self.data.write().insert(
            key.to_string(),
            Entry {
                value,
                expires_at: Some(Instant::now() + ttl),
            },
        );
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.data.write().remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        Ok(self.data.read().contains_key(key))
    }

    async fn health(&self) -> Result<HealthStatus, StorageError> {
        Ok(HealthStatus {
            healthy: true,
            backend: "memory".to_string(),
            message: "OK".to_string(),
        })
    }

    fn backend(&self) -> BackendKind {
        BackendKind::Memory
    }
}
