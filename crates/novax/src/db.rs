//! Database connection management

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::info;

/// Database configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub max_lifetime_seconds: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://novax:novax@localhost:5432/novax".to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout_seconds: 5,
            idle_timeout_seconds: 600,
            max_lifetime_seconds: 1800,
        }
    }
}

/// Create a PostgreSQL connection pool
pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    info!(
        url = %mask_url(&config.url),
        max_connections = config.max_connections,
        "Creating PostgreSQL pool"
    );

    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(Duration::from_secs(config.connect_timeout_seconds))
        .idle_timeout(Duration::from_secs(config.idle_timeout_seconds))
        .max_lifetime(Duration::from_secs(config.max_lifetime_seconds))
        .connect(&config.url)
        .await
}

/// Mask password in URL for logging
fn mask_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(scheme_end) = url.find("://") {
            let scheme = &url[..scheme_end + 3];
            let rest = &url[at_pos + 1..];
            return format!("{}***@{}", scheme, rest);
        }
    }
    url.to_string()
}

/// Run pending migrations from a directory
pub async fn run_migrations(pool: &PgPool, migrations_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    use novax_migrate::MigrationRunner;

    let path = std::path::Path::new(migrations_dir);
    if !path.exists() {
        tracing::warn!(dir = %migrations_dir, "Migrations directory not found, skipping");
        return Ok(());
    }

    let migrations = MigrationRunner::load_from_dir(path).await?;
    let runner = MigrationRunner::new(pool.clone());
    let report = runner.run(&migrations).await?;

    if report.has_changes() {
        for (version, name) in &report.applied {
            info!(version, name = %name, "Applied migration");
        }
    } else {
        info!("Database is up to date (no pending migrations)");
    }

    Ok(())
}
