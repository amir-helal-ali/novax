//! NovaX Network
//!
//! Networking abstraction layer. v0.1 supports HTTP/1.1 and HTTP/2 via hyper/axum.
//! Future versions will add HTTP/3 (QUIC), WebSocket, SSE.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use tracing::info;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub workers: usize,
    pub request_timeout: Duration,
    pub max_connections: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:3000".parse().unwrap(),
            workers: num_cpus(),
            request_timeout: Duration::from_secs(30),
            max_connections: 10000,
        }
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Run the HTTP server with the given router and configuration.
pub async fn serve(router: Router, config: ServerConfig) -> Result<(), ServerError> {
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .map_err(|e| ServerError::Bind(e.to_string()))?;

    info!(
        addr = %config.bind_addr,
        workers = config.workers,
        "NovaX server listening"
    );

    axum::serve(listener, router)
        .await
        .map_err(|e| ServerError::Runtime(e.to_string()))?;

    Ok(())
}

/// Server error types
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("bind error: {0}")]
    Bind(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}

/// Connection info available to handlers
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_addr: Option<SocketAddr>,
    pub protocol: Protocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http1,
    Http2,
    Http3,
    WebSocket,
    Sse,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http1 => "http/1.1",
            Self::Http2 => "http/2",
            Self::Http3 => "http/3",
            Self::WebSocket => "websocket",
            Self::Sse => "sse",
        }
    }
}
