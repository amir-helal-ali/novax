//! NovaX Example Application
//!
//! This is a sample NovaX app showing how to use the platform.
//! Run with: `cargo run -p novax-app` or `docker compose up`.

use novax::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize logging
    novax::observability::init_logging(&std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()));

    info!("NovaX application starting (v{})", novax::version());

    // Build the app with default configuration
    let app = App::new();

    // Get bind address from environment or use default
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{}:{}", host, port);

    info!("Server starting on http://{}", addr);

    // Run the server
    if let Err(e) = app.serve(&addr).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
