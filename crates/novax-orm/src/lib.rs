//! NovaX ORM
//!
//! Provides strongly-typed database queries on top of PostgreSQL.
//! v0.2 includes a Repository pattern with CRUD operations.
//!
//! ## Example
//! ```rust,no_run
//! use novax_orm::{Repository, Entity};
//! use serde::{Serialize, Deserialize};
//! use uuid::Uuid;
//! use chrono::Utc;
//!
//! #[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
//! pub struct User {
//!     pub id: Uuid,
//!     pub email: String,
//!     pub name: String,
//!     pub created_at: chrono::DateTime<Utc>,
//! }
//!
//! impl Entity for User {
//!     const TABLE: &'static str = "users";
//!     type Id = Uuid;
//!     fn id(&self) -> &Self::Id { &self.id }
//! }
//! ```

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::postgres::PgPool;
use thiserror::Error;
use uuid::Uuid;

pub use sqlx;
pub use novax_storage;

/// Error type for ORM operations
#[derive(Debug, Error)]
pub enum OrmError {
    #[error("entity not found")]
    NotFound,
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
}

impl From<sqlx::Error> for OrmError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => OrmError::NotFound,
            other => OrmError::Database(other.to_string()),
        }
    }
}

impl From<serde_json::Error> for OrmError {
    fn from(e: serde_json::Error) -> Self {
        OrmError::Serialization(e.to_string())
    }
}

/// Trait that defines a database entity
pub trait Entity: Send + Sync + Serialize + DeserializeOwned + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + 'static {
    /// Table name in the database
    const TABLE: &'static str;

    /// Type of the primary key
    type Id: Send + Sync + Clone;

    /// Get the primary key value
    fn id(&self) -> &Self::Id;
}

/// Generic repository for CRUD operations on an Entity
pub struct Repository<T: Entity> {
    pool: PgPool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Entity> Repository<T> {
    /// Create a new repository bound to a connection pool
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Find an entity by ID (UUID assumed)
    pub async fn find_by_id(&self, id: Uuid) -> Result<T, OrmError> {
        let query = format!("SELECT * FROM {} WHERE id = $1", T::TABLE);
        let row = sqlx::query_as::<_, T>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.ok_or(OrmError::NotFound)
    }

    /// Find all entities (with optional limit)
    pub async fn find_all(&self, limit: Option<i64>) -> Result<Vec<T>, OrmError> {
        let query = if limit.is_some() {
            format!("SELECT * FROM {} ORDER BY created_at DESC LIMIT $1", T::TABLE)
        } else {
            format!("SELECT * FROM {} ORDER BY created_at DESC", T::TABLE)
        };

        if let Some(l) = limit {
            Ok(sqlx::query_as::<_, T>(&query)
                .bind(l)
                .fetch_all(&self.pool)
                .await?)
        } else {
            Ok(sqlx::query_as::<_, T>(&query)
                .fetch_all(&self.pool)
                .await?)
        }
    }

    /// Count entities
    pub async fn count(&self) -> Result<i64, OrmError> {
        let query = format!("SELECT COUNT(*) as count FROM {}", T::TABLE);
        let row: (i64,) = sqlx::query_as(&query)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Delete an entity by ID
    pub async fn delete(&self, id: Uuid) -> Result<bool, OrmError> {
        let query = format!("DELETE FROM {} WHERE id = $1", T::TABLE);
        let result = sqlx::query(&query)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Check if entity exists
    pub async fn exists(&self, id: Uuid) -> Result<bool, OrmError> {
        let query = format!("SELECT 1 FROM {} WHERE id = $1", T::TABLE);
        let row: Option<(i32,)> = sqlx::query_as(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    /// Begin a transaction
    pub async fn begin(&self) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, OrmError> {
        self.pool.begin().await.map_err(OrmError::from)
    }
}

/// Async transactional operation
#[async_trait]
pub trait Transactional {
    type Output;
    async fn run(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<Self::Output, OrmError>;
}

/// Pagination parameters
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 { 1 }
fn default_per_page() -> u32 { 20 }

impl Default for Pagination {
    fn default() -> Self {
        Self { page: 1, per_page: 20 }
    }
}

impl Pagination {
    pub fn limit(&self) -> i64 {
        self.per_page as i64
    }

    pub fn offset(&self) -> i64 {
        ((self.page.saturating_sub(1)) * self.per_page) as i64
    }
}

/// Paginated result
#[derive(Debug, serde::Serialize)]
pub struct PaginatedResult<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

impl<T: Entity> Repository<T> {
    /// Find entities with pagination
    pub async fn find_paginated(&self, pagination: Pagination) -> Result<PaginatedResult<T>, OrmError> {
        let total = self.count().await?;
        let total_pages = if total == 0 {
            1
        } else {
            ((total as u32) + pagination.per_page - 1) / pagination.per_page
        };

        let query = format!(
            "SELECT * FROM {} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            T::TABLE
        );
        let items = sqlx::query_as::<_, T>(&query)
            .bind(pagination.limit())
            .bind(pagination.offset())
            .fetch_all(&self.pool)
            .await?;

        Ok(PaginatedResult {
            items,
            total,
            page: pagination.page,
            per_page: pagination.per_page,
            total_pages,
        })
    }
}
