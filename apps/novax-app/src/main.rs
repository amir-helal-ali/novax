//! NovaX Example Application
//!
//! Demonstrates the platform with PostgreSQL + Authentication + Posts CRUD.
//!
//! Environment variables:
//! - HOST: bind host (default 0.0.0.0)
//! - PORT: bind port (default 3000)
//! - RUST_LOG: log level (default info)
//! - DATABASE_URL: PostgreSQL connection string
//! - JWT_SECRET: secret for JWT signing (IMPORTANT in production!)

use novax::prelude::*;
use tracing::{info, error};

#[tokio::main]
async fn main() {
    // Initialize logging
    novax::observability::init_logging(&std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()));

    info!("NovaX application starting (v{})", novax::version());

    // Database configuration from environment
    let db_config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://novax:novax@localhost:5432/novax".to_string()),
        max_connections: std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
        min_connections: 1,
        connect_timeout_seconds: 5,
        idle_timeout_seconds: 600,
        max_lifetime_seconds: 1800,
    };

    // Auth configuration from environment
    let auth_config = AuthConfig::default();

    // Build the app with database + auth
    let app = App::new()
        .with_database(db_config)
        .with_auth(auth_config);

    // Initialize (connect DB + run migrations + init auth)
    let app = match app.initialize().await {
        Ok(app) => app,
        Err(e) => {
            error!("Failed to initialize app: {}", e);
            error!("Continuing without database — API endpoints will return 503");
            App::new()
        }
    };

    // Get bind address
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{}:{}", host, port);

    info!("Server starting on http://{}", addr);

    // Run the server
    if let Err(e) = app.serve(&addr).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}
