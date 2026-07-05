//! NovaX Router
//!
//! HTTP router with type-safe route definitions.
//! v0.1 uses axum as backend. Future versions will have a native NovaX router.

pub use axum::{
    self,
    Router,
    routing::{get, post, put, delete, patch},
    extract::{Path, Query, State},
    response::{Json, IntoResponse, Response, Html},
    http::{StatusCode, Method, HeaderMap, Uri},
    body::Body,
};

use std::sync::Arc;
use std::time::Instant;

use tower_http::trace::TraceLayer;
use tower_http::cors::CorsLayer;
use tower_http::compression::CompressionLayer;
use serde::{Deserialize, Serialize};

/// Application shared state
#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
    pub config: Arc<RouterConfig>,
}

/// Router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    pub enable_cors: bool,
    pub enable_compression: bool,
    pub enable_tracing: bool,
    pub max_body_size: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            enable_cors: true,
            enable_compression: true,
            enable_tracing: true,
            max_body_size: 2 * 1024 * 1024,
        }
    }
}

/// Add NovaX default middleware to a router
pub fn with_defaults<S>(router: Router<S>, config: &RouterConfig) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let mut r = router;

    if config.enable_tracing {
        r = r.layer(TraceLayer::new_for_http());
    }
    if config.enable_cors {
        r = r.layer(CorsLayer::permissive());
    }
    if config.enable_compression {
        r = r.layer(CompressionLayer::new());
    }

    r
}

/// Helper to build a JSON response
pub fn json_response<T: serde::Serialize>(status: StatusCode, body: &T) -> Response {
    (status, Json(body)).into_response()
}

/// Helper for error responses
pub fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({
        "error": {
            "code": status.as_u16(),
            "message": message,
        }
    });
    (status, Json(body)).into_response()
}
