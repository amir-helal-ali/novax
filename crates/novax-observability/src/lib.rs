//! NovaX Observability
//!
//! Built-in structured logging, metrics, and tracing.
//! Zero external services required for basic observability.

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;

/// Initialize the global tracing subscriber.
/// Call this once at startup.
pub fn init_logging(level: &str) {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact();

    let json_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_filter(filter.clone());

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(json_layer)
        .init();
}

/// Counter metric
#[derive(Debug)]
pub struct Counter {
    name: &'static str,
    help: &'static str,
    labels: DashMap<Vec<(String, String)>, u64>,
}

impl Counter {
    pub fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            labels: DashMap::new(),
        }
    }

    pub fn inc(&self, labels: &[(&str, &str)]) {
        let key: Vec<(String, String)> = labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        *self.labels.entry(key).or_insert(0) += 1;
    }

    pub fn inc_by(&self, labels: &[(&str, &str)], delta: u64) {
        let key: Vec<(String, String)> = labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        *self.labels.entry(key).or_insert(0) += delta;
    }

    pub fn snapshot(&self) -> Vec<(Vec<(String, String)>, u64)> {
        self.labels
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect()
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn help(&self) -> &'static str {
        self.help
    }
}

/// Histogram metric
#[derive(Debug)]
pub struct Histogram {
    name: &'static str,
    help: &'static str,
    buckets: &'static [f64],
    counts: DashMap<Vec<(String, String)>, Vec<u64>>,
    sums: DashMap<Vec<(String, String)>, f64>,
}

impl Histogram {
    pub fn new(name: &'static str, help: &'static str, buckets: &'static [f64]) -> Self {
        Self {
            name,
            help,
            buckets,
            counts: DashMap::new(),
            sums: DashMap::new(),
        }
    }

    pub fn observe(&self, labels: &[(&str, &str)], value: f64) {
        let key: Vec<(String, String)> = labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let mut counts = self.counts.entry(key.clone()).or_insert_with(|| {
            vec![0; self.buckets.len() + 1]  // +1 for +Inf
        });

        for (i, &bucket) in self.buckets.iter().enumerate() {
            if value <= bucket {
                counts[i] += 1;
            }
        }
        *counts.last_mut().unwrap() += 1;  // +Inf bucket

        let mut sum = self.sums.entry(key).or_insert(0.0);
        *sum += value;
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// Global metrics registry
pub static REGISTRY: Lazy<Arc<Registry>> = Lazy::new(|| Arc::new(Registry::new()));

pub struct Registry {
    counters: Mutex<Vec<Arc<Counter>>>,
    histograms: Mutex<Vec<Arc<Histogram>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            counters: Mutex::new(Vec::new()),
            histograms: Mutex::new(Vec::new()),
        }
    }

    pub fn register_counter(&self, counter: Arc<Counter>) {
        self.counters.lock().push(counter);
    }

    pub fn register_histogram(&self, histogram: Arc<Histogram>) {
        self.histograms.lock().push(histogram);
    }

    pub fn export_prometheus(&self) -> String {
        let mut buf = String::new();

        for counter in self.counters.lock().iter() {
            buf.push_str(&format!("# HELP {} {}\n", counter.name(), counter.help()));
            buf.push_str(&format!("# TYPE {} counter\n", counter.name()));
            for (labels, value) in counter.snapshot() {
                let label_str = labels
                    .iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v.replace('"', "\\\"")))
                    .collect::<Vec<_>>()
                    .join(",");
                buf.push_str(&format!("{}{{{}}} {}\n", counter.name(), label_str, value));
            }
        }

        for histogram in self.histograms.lock().iter() {
            buf.push_str(&format!("# HELP {} {}\n", histogram.name(), histogram.help));
            buf.push_str(&format!("# TYPE {} histogram\n", histogram.name()));
            // ... simplified export
        }

        buf
    }
}

/// Built-in HTTP request counter
pub static HTTP_REQUESTS_TOTAL: Lazy<Arc<Counter>> = Lazy::new(|| {
    let c = Arc::new(Counter::new(
        "novax_http_requests_total",
        "Total HTTP requests",
    ));
    REGISTRY.register_counter(c.clone());
    c
});

/// Built-in HTTP request duration histogram
pub static HTTP_REQUEST_DURATION: Lazy<Arc<Histogram>> = Lazy::new(|| {
    let h = Arc::new(Histogram::new(
        "novax_http_request_duration_seconds",
        "HTTP request duration",
        &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
    ));
    REGISTRY.register_histogram(h.clone());
    h
});

/// Timer for measuring code execution time
pub struct Timer {
    start: Instant,
    name: &'static str,
}

impl Timer {
    pub fn start(name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            name,
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        tracing::debug!(timer = self.name, duration_ms = self.elapsed().as_millis() as u64);
    }
}

/// Health status of a subsystem
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub status: String,  // "healthy" or "unhealthy"
    pub subsystem: String,
    pub message: Option<String>,
    pub timestamp: String,
}

impl HealthStatus {
    pub fn healthy(subsystem: &str) -> Self {
        Self {
            status: "healthy".to_string(),
            subsystem: subsystem.to_string(),
            message: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn unhealthy(subsystem: &str, msg: &str) -> Self {
        Self {
            status: "unhealthy".to_string(),
            subsystem: subsystem.to_string(),
            message: Some(msg.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Overall system health
#[derive(Debug, Serialize)]
pub struct SystemHealth {
    pub status: String,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub subsystems: Vec<HealthStatus>,
}

static START_TIME: Lazy<Instant> = Lazy::new(Instant::now);

pub fn system_health() -> SystemHealth {
    let subsystems = vec![
        HealthStatus::healthy("runtime"),
        HealthStatus::healthy("network"),
        HealthStatus::healthy("router"),
    ];

    let all_healthy = subsystems.iter().all(|s| s.status == "healthy");

    SystemHealth {
        status: if all_healthy { "healthy".to_string() } else { "degraded".to_string() },
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: START_TIME.elapsed().as_secs(),
        subsystems,
    }
}
