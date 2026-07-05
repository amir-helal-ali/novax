//! NovaX Application builder

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{Router, routing::get, response::Html};
use novax_network::{ServerConfig, serve, ServerError};
use novax_observability::system_health;
use novax_router::{AppState, with_defaults};
use serde::{Serialize, Deserialize};
use tracing::info;

use crate::config::NovaXConfig;

/// NovaX application
pub struct App {
    pub config: NovaXConfig,
    pub state: AppState,
}

impl App {
    /// Create a new NovaX application with default configuration
    pub fn new() -> Self {
        let config = NovaXConfig::default();
        Self::with_config(config)
    }

    /// Create a new NovaX application with the given configuration
    pub fn with_config(config: NovaXConfig) -> Self {
        let state = AppState {
            start_time: Instant::now(),
            config: Arc::new(config.router.clone()),
        };
        Self { config, state }
    }

    /// Run the application on the given address
    pub async fn serve(self, addr: &str) -> Result<(), ServerError> {
        let bind_addr: SocketAddr = addr
            .parse()
            .map_err(|e: std::net::AddrParseError| ServerError::Bind(e.to_string()))?;

        let server_config = ServerConfig {
            bind_addr,
            ..Default::default()
        };

        let router = build_default_router(self.state);

        info!(
            version = env!("CARGO_PKG_VERSION"),
            addr = %bind_addr,
            "NovaX application starting"
        );

        serve(router, server_config).await
    }

    /// Run the application with the configured address
    pub async fn run(self) -> Result<(), ServerError> {
        let addr = self.config.server.bind_addr.to_string();
        self.serve(&addr).await
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default router with health, info, and dashboard routes
fn build_default_router(state: AppState) -> Router {
    let router: Router<AppState> = Router::new()
        .route("/", get(dashboard))
        .route("/health", get(health_handler))
        .route("/api/health", get(api_health_handler))
        .route("/api/info", get(api_info_handler))
        .route("/api/version", get(api_version_handler))
        .route("/api/metrics", get(metrics_handler))
        .route("/static/*path", get(static_handler));

    let router = with_defaults(router, &state.config);
    router.with_state(state)
}

/// GET / — Dashboard HTML page
async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// GET /health — Health check endpoint
async fn health_handler() -> axum::Json<novax_observability::SystemHealth> {
    axum::Json(system_health())
}

/// GET /api/health — API health check
async fn api_health_handler() -> axum::Json<novax_observability::SystemHealth> {
    axum::Json(system_health())
}

#[derive(Serialize, Deserialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    description: &'static str,
    homepage: &'static str,
    rust_version: &'static str,
    features: Vec<&'static str>,
}

/// GET /api/info — Application info
async fn api_info_handler() -> axum::Json<AppInfo> {
    axum::Json(AppInfo {
        name: "NovaX",
        version: env!("CARGO_PKG_VERSION"),
        description: "A next-generation full-stack web platform built entirely in Rust",
        homepage: "https://github.com/amir-helal-ali/novax",
        rust_version: "1.82+",
        features: vec![
            "Rust end-to-end",
            "HTTP/1.1 + HTTP/2",
            "Async runtime (tokio-based for v0.1)",
            "Built-in observability",
            "Multi-backend storage",
            "Docker-ready",
            "Type-safe routing",
        ],
    })
}

/// GET /api/version — Just the version string
async fn api_version_handler() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// GET /api/metrics — Prometheus metrics
async fn metrics_handler() -> String {
    novax_observability::REGISTRY.export_prometheus()
}

/// Static file handler (basic)
async fn static_handler(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    let path = path.trim_start_matches('/');
    let file_path = format!("static/{}", path);

    match tokio::fs::read_to_string(&file_path).await {
        Ok(content) => {
            let mime = mime_for_extension(&file_path);
            ([(axum::http::header::CONTENT_TYPE, mime)], content).into_response()
        }
        Err(_) => (axum::http::StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}

fn mime_for_extension(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "text/plain; charset=utf-8"
    }
}

use axum::response::IntoResponse;

const DASHBOARD_HTML: &str = include_str!("../../../static/index.html");
