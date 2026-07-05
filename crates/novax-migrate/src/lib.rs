//! NovaX Migration Engine
//!
//! Provides database schema migrations with versioning, rollback, and validation.
//! v0.2 supports PostgreSQL via sqlx.

use std::path::Path;

use chrono::{DateTime, Utc};
use thiserror::Error;
use tracing::{info, warn};

#[cfg(feature = "postgres")]
use sqlx::postgres::PgPool;

/// Migration error types
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("database error: {0}")]
    Database(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("migration not found: {0}")]
    NotFound(String),
    #[error("migration already applied: {0}")]
    AlreadyApplied(String),
    #[error("destructive operation detected: {0}")]
    Destructive(String),
    #[error("invalid migration file: {0}")]
    InvalidFile(String),
}

#[cfg(feature = "postgres")]
impl From<sqlx::Error> for MigrationError {
    fn from(e: sqlx::Error) -> Self {
        MigrationError::Database(e.to_string())
    }
}

impl From<std::io::Error> for MigrationError {
    fn from(e: std::io::Error) -> Self {
        MigrationError::Io(e.to_string())
    }
}

/// A single migration definition
#[derive(Debug, Clone)]
pub struct Migration {
    /// Version number (e.g. 1, 2, 3...)
    pub version: i64,
    /// Human-readable name
    pub name: String,
    /// SQL to apply this migration
    pub up_sql: String,
    /// SQL to roll back this migration
    pub down_sql: String,
    /// Whether this migration contains destructive operations
    pub destructive: bool,
}

impl Migration {
    /// Create a new migration
    pub fn new(version: i64, name: impl Into<String>, up_sql: impl Into<String>, down_sql: impl Into<String>) -> Self {
        let up_sql = up_sql.into();
        let destructive = detect_destructive(&up_sql);
        Self {
            version,
            name: name.into(),
            up_sql,
            down_sql: down_sql.into(),
            destructive,
        }
    }

    /// Parse a migration SQL file with `-- +migrate Up` and `-- +migrate Down` markers
    pub fn parse_sql(version: i64, name: &str, content: &str) -> Result<Self, MigrationError> {
        let mut up_sql = String::new();
        let mut down_sql = String::new();
        let mut in_up = false;
        let mut in_down = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("-- +migrate Up") {
                in_up = true;
                in_down = false;
                continue;
            } else if trimmed.eq_ignore_ascii_case("-- +migrate Down") {
                in_up = false;
                in_down = true;
                continue;
            }

            if in_up {
                up_sql.push_str(line);
                up_sql.push('\n');
            } else if in_down {
                down_sql.push_str(line);
                down_sql.push('\n');
            }
        }

        if up_sql.is_empty() {
            return Err(MigrationError::InvalidFile(
                format!("migration {} has no Up section", name),
            ));
        }

        Ok(Self::new(version, name, up_sql, down_sql))
    }
}

/// Detect destructive SQL operations
fn detect_destructive(sql: &str) -> bool {
    let upper = sql.to_uppercase();
    upper.contains("DROP TABLE")
        || upper.contains("DROP COLUMN")
        || upper.contains("TRUNCATE")
        || upper.contains("DELETE FROM")
}

/// Database row for the migrations tracking table
#[derive(Debug, sqlx::FromRow)]
struct MigrationRecord {
    version: i64,
    name: String,
    applied_at: DateTime<Utc>,
}

/// Migration runner
#[cfg(feature = "postgres")]
pub struct MigrationRunner {
    pool: PgPool,
    allow_destructive: bool,
}

#[cfg(feature = "postgres")]
impl MigrationRunner {
    /// Create a new migration runner
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            allow_destructive: false,
        }
    }

    /// Allow destructive migrations (DROP TABLE, TRUNCATE, etc.)
    pub fn allow_destructive(mut self) -> Self {
        self.allow_destructive = true;
        self
    }

    /// Ensure the migrations tracking table exists
    async fn ensure_table(&self) -> Result<(), MigrationError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _novax_migrations (
                version BIGINT PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List all applied migrations
    pub async fn list_applied(&self) -> Result<Vec<(i64, String)>, MigrationError> {
        self.ensure_table().await?;
        let rows: Vec<MigrationRecord> = sqlx::query_as(
            "SELECT version, name, applied_at FROM _novax_migrations ORDER BY version",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| (r.version, r.name)).collect())
    }

    /// Apply all pending migrations
    pub async fn run(&self, migrations: &[Migration]) -> Result<MigrationReport, MigrationError> {
        self.ensure_table().await?;
        let applied_set = self.list_applied().await?;
        let applied_versions: std::collections::HashSet<i64> =
            applied_set.iter().map(|(v, _)| *v).collect();

        let mut report = MigrationReport::default();

        // Sort migrations by version
        let mut sorted = migrations.to_vec();
        sorted.sort_by_key(|m| m.version);

        for migration in sorted {
            if applied_versions.contains(&migration.version) {
                report.skipped.push(migration.name.clone());
                continue;
            }

            // Validate destructive operations
            if migration.destructive && !self.allow_destructive {
                return Err(MigrationError::Destructive(format!(
                    "migration {} (v{}) contains destructive operations. \
                     Use `.allow_destructive()` to permit.",
                    migration.name, migration.version
                )));
            }

            info!(
                version = migration.version,
                name = %migration.name,
                "Applying migration"
            );

            // Apply in a transaction
            let mut tx = self.pool.begin().await?;
            let apply_result: Result<(), MigrationError> = async {
                // Use raw_sql to support multi-statement SQL scripts
                // (CREATE TABLE + CREATE INDEX + ... in one migration file)
                sqlx::raw_sql(&migration.up_sql)
                    .execute(&mut *tx)
                    .await?;

                sqlx::query("INSERT INTO _novax_migrations (version, name) VALUES ($1, $2)")
                    .bind(migration.version)
                    .bind(&migration.name)
                    .execute(&mut *tx)
                    .await?;
                Ok(())
            }
            .await;

            match apply_result {
                Ok(()) => {
                    tx.commit().await?;
                    report.applied.push((migration.version, migration.name.clone()));
                }
                Err(e) => {
                    let _ = tx.rollback().await;
                    return Err(MigrationError::Database(format!(
                        "migration {} v{} failed: {}",
                        migration.name, migration.version, e
                    )));
                }
            }
        }

        if report.applied.is_empty() {
            info!("No pending migrations");
        } else {
            info!("Applied {} migration(s)", report.applied.len());
        }

        Ok(report)
    }

    /// Roll back the most recently applied migration
    pub async fn rollback_last(&self, migrations: &[Migration]) -> Result<Option<(i64, String)>, MigrationError> {
        self.ensure_table().await?;
        let applied = self.list_applied().await?;

        let Some((last_version, last_name)) = applied.last() else {
            warn!("No migrations to rollback");
            return Ok(None);
        };

        // Find the migration definition
        let migration = migrations
            .iter()
            .find(|m| m.version == *last_version)
            .ok_or_else(|| MigrationError::NotFound(format!("migration v{} not in source", last_version)))?;

        if migration.down_sql.is_empty() {
            return Err(MigrationError::InvalidFile(format!(
                "migration {} has no Down section",
                migration.name
            )));
        }

        info!(
            version = migration.version,
            name = %migration.name,
            "Rolling back migration"
        );

        let mut tx = self.pool.begin().await?;
        let rollback_result: Result<(), MigrationError> = async {
            // Use raw_sql for multi-statement support
            sqlx::raw_sql(&migration.down_sql)
                .execute(&mut *tx)
                .await?;

            sqlx::query("DELETE FROM _novax_migrations WHERE version = $1")
                .bind(migration.version)
                .execute(&mut *tx)
                .await?;
            Ok(())
        }
        .await;

        match rollback_result {
            Ok(()) => {
                tx.commit().await?;
                Ok(Some((*last_version, last_name.clone())))
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(MigrationError::Database(format!("rollback failed: {}", e)))
            }
        }
    }

    /// Load migrations from a directory
    /// Expects files named like: `001_create_users.sql`, `002_add_posts.sql`
    pub async fn load_from_dir(dir: &Path) -> Result<Vec<Migration>, MigrationError> {
        let mut migrations = Vec::new();

        let mut entries = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "sql"))
            .collect::<Vec<_>>();

        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| MigrationError::InvalidFile(format!("invalid filename: {:?}", path)))?;

            // Parse version and name from filename: "001_create_users.sql"
            let (version, name) = parse_migration_filename(filename)?;
            let content = std::fs::read_to_string(&path)?;
            let migration = Migration::parse_sql(version, &name, &content)?;
            migrations.push(migration);
        }

        Ok(migrations)
    }
}

/// Parse a migration filename like "001_create_users.sql"
fn parse_migration_filename(filename: &str) -> Result<(i64, String), MigrationError> {
    let stem = filename.strip_suffix(".sql").unwrap_or(filename);
    let parts: Vec<&str> = stem.splitn(2, '_').collect();
    if parts.len() != 2 {
        return Err(MigrationError::InvalidFile(format!(
            "filename should be like '001_name.sql', got: {}",
            filename
        )));
    }
    let version = parts[0].parse::<i64>().map_err(|_| {
        MigrationError::InvalidFile(format!("invalid version number in: {}", filename))
    })?;
    Ok((version, parts[1].to_string()))
}

/// Report of a migration run
#[derive(Debug, Default)]
pub struct MigrationReport {
    /// Migrations that were applied
    pub applied: Vec<(i64, String)>,
    /// Migrations that were skipped (already applied)
    pub skipped: Vec<String>,
}

impl MigrationReport {
    pub fn has_changes(&self) -> bool {
        !self.applied.is_empty()
    }
}

#[cfg(not(feature = "postgres"))]
pub struct MigrationRunner;

#[cfg(not(feature = "postgres"))]
impl MigrationRunner {
    pub fn new(_pool: ()) -> Self {
        Self
    }
}
