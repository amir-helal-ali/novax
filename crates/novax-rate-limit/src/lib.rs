//! NovaX Rate Limiting
//!
//! Token bucket rate limiter with per-IP and per-user limits.
//! Configurable via `RateLimitConfig`.
//!
//! ## Example
//! ```rust,no_run
//! use novax_rate_limit::{RateLimiter, RateLimitConfig};
//!
//! let config = RateLimitConfig::default();
//! let limiter = RateLimiter::new(config);
//! // In middleware: limiter.check("192.168.1.1").await
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per window (default: 100)
    pub max_requests: u32,
    /// Window duration in seconds (default: 60 = 1 minute)
    pub window_seconds: u64,
    /// Whether to enable rate limiting at all
    pub enabled: bool,
    /// Whitelisted IPs (no rate limiting)
    pub whitelist: Vec<String>,
    /// Burst capacity (token bucket — default: same as max_requests)
    pub burst: Option<u32>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window_seconds: 60,
            enabled: true,
            whitelist: vec!["127.0.0.1".to_string(), "::1".to_string()],
            burst: None,
        }
    }
}

impl RateLimitConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(v) = std::env::var("RATE_LIMIT_MAX_REQUESTS") {
            if let Ok(n) = v.parse() { config.max_requests = n; }
        }
        if let Ok(v) = std::env::var("RATE_LIMIT_WINDOW_SECONDS") {
            if let Ok(n) = v.parse() { config.window_seconds = n; }
        }
        if let Ok(v) = std::env::var("RATE_LIMIT_ENABLED") {
            config.enabled = matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on");
        }
        if let Ok(v) = std::env::var("RATE_LIMIT_WHITELIST") {
            config.whitelist = v.split(',').map(|s| s.trim().to_string()).collect();
        }
        config
    }

    /// Burst capacity (defaults to max_requests if not set)
    pub fn burst_capacity(&self) -> u32 {
        self.burst.unwrap_or(self.max_requests)
    }
}

/// Per-key state (token bucket)
#[derive(Debug, Clone)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

impl BucketState {
    fn new(capacity: u32) -> Self {
        Self {
            tokens: capacity as f64,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self, capacity: u32, refill_rate: f64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let tokens_to_add = elapsed * refill_rate;
        self.tokens = (self.tokens + tokens_to_add).min(capacity as f64);
        self.last_refill = now;
    }

    fn try_consume(&mut self, n: f64) -> bool {
        if self.tokens >= n {
            self.tokens -= n;
            true
        } else {
            false
        }
    }
}

/// Rate limiter using token bucket algorithm
#[derive(Clone)]
pub struct RateLimiter {
    config: Arc<RateLimitConfig>,
    buckets: Arc<DashMap<String, BucketState>>,
    refill_rate: f64,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        let refill_rate = config.max_requests as f64 / config.window_seconds as f64;
        let capacity = config.burst_capacity();
        debug!(
            max_requests = config.max_requests,
            window_seconds = config.window_seconds,
            burst = capacity,
            refill_rate,
            "Rate limiter initialized"
        );
        Self {
            config: Arc::new(config),
            buckets: Arc::new(DashMap::new()),
            refill_rate,
        }
    }

    /// Check if a request is allowed for the given key (IP or user ID)
    pub fn check(&self, key: &str) -> RateLimitResult {
        if !self.config.enabled {
            return RateLimitResult::Allowed;
        }

        // Check whitelist
        if self.config.whitelist.iter().any(|ip| ip == key) {
            return RateLimitResult::Allowed;
        }

        let capacity = self.config.burst_capacity();
        let mut entry = self.buckets
            .entry(key.to_string())
            .or_insert_with(|| BucketState::new(capacity));

        let bucket = entry.value_mut();
        bucket.refill(capacity, self.refill_rate);

        if bucket.try_consume(1.0) {
            RateLimitResult::Allowed
        } else {
            let retry_after = ((1.0 - bucket.tokens) / self.refill_rate).ceil() as u64;
            RateLimitResult::Denied {
                retry_after_seconds: retry_after.max(1),
                limit: self.config.max_requests,
                remaining: 0,
            }
        }
    }

    /// Cleanup expired buckets (call periodically)
    pub fn cleanup(&self) {
        let cutoff = Instant::now() - Duration::from_secs(self.config.window_seconds * 2);
        self.buckets.retain(|_, state| state.last_refill > cutoff);
    }

    /// Get current configuration
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }
}

/// Rate limit check result
#[derive(Debug)]
pub enum RateLimitResult {
    Allowed,
    Denied {
        retry_after_seconds: u64,
        limit: u32,
        remaining: u32,
    },
}

/// Extract client IP from request
pub fn extract_client_ip(req: &Request) -> String {
    // Try X-Forwarded-For header first (when behind a proxy)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(s) = forwarded.to_str() {
            if let Some(first_ip) = s.split(',').next() {
                return first_ip.trim().to_string();
            }
        }
    }
    // Try X-Real-IP
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            return s.trim().to_string();
        }
    }
    // Fallback: connection info
    "unknown".to_string()
}

/// Axum middleware: apply rate limiting per-IP
pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    req: Request,
    next: Next,
) -> Response {
    let ip = extract_client_ip(&req);

    match limiter.check(&ip) {
        RateLimitResult::Allowed => next.run(req).await,
        RateLimitResult::Denied { retry_after_seconds, limit, remaining } => {
            let body = serde_json::json!({
                "error": {
                    "code": 429,
                    "message": "Too Many Requests",
                    "retry_after_seconds": retry_after_seconds,
                }
            });
            let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
            response.headers_mut().insert(
                "x-ratelimit-limit",
                limit.to_string().parse().unwrap(),
            );
            response.headers_mut().insert(
                "x-ratelimit-remaining",
                remaining.to_string().parse().unwrap(),
            );
            response.headers_mut().insert(
                "retry-after",
                retry_after_seconds.to_string().parse().unwrap(),
            );
            response
        }
    }
}

/// Background cleanup task — call once at startup
pub fn spawn_cleanup_task(limiter: RateLimiter) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            limiter.cleanup();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_allows_within_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 5,
            window_seconds: 60,
            enabled: true,
            whitelist: vec![],
            burst: Some(5),
        });
        for _ in 0..5 {
            assert!(matches!(limiter.check("1.2.3.4"), RateLimitResult::Allowed));
        }
        // 6th should be denied
        assert!(matches!(limiter.check("1.2.3.4"), RateLimitResult::Denied { .. }));
    }

    #[test]
    fn test_whitelist_bypasses() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window_seconds: 60,
            enabled: true,
            whitelist: vec!["127.0.0.1".to_string()],
            burst: Some(1),
        });
        // Whitelisted IP can exceed limit
        for _ in 0..10 {
            assert!(matches!(limiter.check("127.0.0.1"), RateLimitResult::Allowed));
        }
    }

    #[test]
    fn test_disabled_allows_all() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_requests: 1,
            window_seconds: 60,
            enabled: false,
            whitelist: vec![],
            burst: Some(1),
        });
        for _ in 0..100 {
            assert!(matches!(limiter.check("any"), RateLimitResult::Allowed));
        }
    }
}
