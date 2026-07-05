//! PostgreSQL storage backend
//!
//! Provides persistent storage using PostgreSQL via sqlx.
//! Suitable for production use.

use std::time::Duration;

use async_trait::async_trait;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::info;

use crate::{BackendKind, HealthStatus, Storage, StorageError};

/// PostgreSQL storage implementation
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    /// Create a new PostgreSQL storage from a connection URL
    pub async fn new(url: &str) -> Result<Self, StorageError> {
        Self::with_config(url, 10, 5, Duration::from_secs(5)).await
    }

    /// Create with custom pool configuration
    pub async fn with_config(
        url: &str,
        max_connections: u32,
        min_connections: u32,
        connect_timeout: Duration,
    ) -> Result<Self, StorageError> {
        info!(
            url = %mask_url(url),
            max_connections,
            min_connections,
            "Connecting to PostgreSQL"
        );

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .acquire_timeout(connect_timeout)
            .connect(url)
            .await
            .map_err(|e| StorageError::Backend(format!("postgres connect: {}", e)))?;

        // Initialize schema if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS novax_kv_store (
                key TEXT PRIMARY KEY,
                value BYTEA NOT NULL,
                expires_at TIMESTAMPTZ NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| StorageError::Backend(format!("create table: {}", e)))?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_novax_kv_expires_at
               ON novax_kv_store (expires_at) WHERE expires_at IS NOT NULL"#,
        )
        .execute(&pool)
        .await
        .map_err(|e| StorageError::Backend(format!("create index: {}", e)))?;

        Ok(Self { pool })
    }

    /// Get a reference to the underlying connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Run a cleanup of expired entries
    pub async fn cleanup_expired(&self) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM novax_kv_store WHERE expires_at IS NOT NULL AND expires_at < NOW()")
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(format!("cleanup: {}", e)))?;
        Ok(result.rows_affected())
    }
}

fn mask_url(url: &str) -> String {
    // Hide password in logs
    if let Some(at_pos) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let scheme = &url[..scheme_end + 3];
            let rest = &url[at_pos + 1..];
            return format!("{}***@{}", scheme, rest);
        }
    }
    url.to_string()
}

#[async_trait]
impl Storage for PostgresStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let row: Option<(Vec<u8>, Option<chrono::DateTime<chrono::Utc>>)> =
            sqlx::query_as("SELECT value, expires_at FROM novax_kv_store WHERE key = $1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| StorageError::Backend(format!("get: {}", e)))?;

        match row {
            None => Ok(None),
            Some((value, Some(expires_at))) => {
                if chrono::Utc::now() >= expires_at {
                    // Expired — delete and return None
                    let _ = sqlx::query("DELETE FROM novax_kv_store WHERE key = $1")
                        .bind(key)
                        .execute(&self.pool)
                        .await;
                    Ok(None)
                } else {
                    Ok(Some(value))
                }
            }
            Some((value, None)) => Ok(Some(value)),
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO novax_kv_store (key, value, expires_at)
             VALUES ($1, $2, NULL)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, expires_at = NULL",
        )
        .bind(key)
        .bind(&value)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Backend(format!("set: {}", e)))?;
        Ok(())
    }

    async fn set_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<(), StorageError> {
        let expires_at = chrono::Utc::now() + chrono::Duration::from_std(ttl).unwrap();
        sqlx::query(
            "INSERT INTO novax_kv_store (key, value, expires_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, expires_at = EXCLUDED.expires_at",
        )
        .bind(key)
        .bind(&value)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Backend(format!("set_with_ttl: {}", e)))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM novax_kv_store WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Backend(format!("delete: {}", e)))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM novax_kv_store WHERE key = $1 AND (expires_at IS NULL OR expires_at > NOW())")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| StorageError::Backend(format!("exists: {}", e)))?;
        Ok(row.is_some())
    }

    async fn health(&self) -> Result<HealthStatus, StorageError> {
        match sqlx::query("SELECT 1").execute(&self.pool).await {
            Ok(_) => Ok(HealthStatus {
                healthy: true,
                backend: "postgres".to_string(),
                message: "OK".to_string(),
            }),
            Err(e) => Ok(HealthStatus {
                healthy: false,
                backend: "postgres".to_string(),
                message: e.to_string(),
            }),
        }
    }

    fn backend(&self) -> BackendKind {
        BackendKind::Postgres
    }
}
