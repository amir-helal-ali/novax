//! NovaX Example Application (v0.4)
//!
//! Full-featured: PostgreSQL + Auth + Rate Limiting + OAuth + Admin Dashboard

use novax::prelude::*;
use tracing::{info, error};

#[tokio::main]
async fn main() {
    novax::observability::init_logging(&std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()));
    info!("NovaX application starting (v{})", novax::version());

    let db_config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://novax:novax@localhost:5432/novax".to_string()),
        max_connections: std::env::var("DB_MAX_CONNECTIONS")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(10),
        min_connections: 1,
        connect_timeout_seconds: 5,
        idle_timeout_seconds: 600,
        max_lifetime_seconds: 1800,
    };

    let auth_config = AuthConfig::default();
    let rate_limit_config = RateLimitConfig::from_env();
    let oauth_config = OAuthConfig::default();

    let is_dev = std::env::var("NOVAX_ENV")
        .map(|e| e == "development")
        .unwrap_or(true);

    let app = App::new()
        .with_database(db_config)
        .with_auth(auth_config)
        .with_rate_limiting(rate_limit_config)
        .with_oauth(oauth_config);

    let app = if is_dev {
        app.dev_mode()
    } else {
        app
    };

    let app = match app.initialize().await {
        Ok(app) => app,
        Err(e) => {
            error!("Failed to initialize: {}", e);
            App::new()
        }
    };

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{}:{}", host, port);

    info!("Server starting on http://{}", addr);

    if let Err(e) = app.serve(&addr).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}
