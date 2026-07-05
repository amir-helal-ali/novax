//! NovaX Runtime
//!
//! Provides an async executor and utilities. For v0.1, this is a thin wrapper
//! around tokio to ensure stability. Future versions (v0.2+) will replace
//! the backend with a native NovaX work-stealing scheduler.

use std::future::Future;
use std::time::Duration;

pub use tokio::{
    self,
    spawn,
    task::yield_now,
    time::sleep,
    time::timeout,
    sync::{Mutex, RwLock, oneshot, mpsc, broadcast},
};

pub use tokio::runtime::{Runtime, Builder};

/// Configuration for the NovaX runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub worker_threads: usize,
    pub blocking_threads: usize,
    pub stack_size: usize,
    pub max_io_events_per_tick: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: num_cpus(),
            blocking_threads: 512,
            stack_size: 2 * 1024 * 1024,
            max_io_events_per_tick: 1024,
        }
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

pub fn build(config: RuntimeConfig) -> Runtime {
    Builder::new_multi_thread()
        .worker_threads(config.worker_threads)
        .max_blocking_threads(config.blocking_threads)
        .thread_stack_size(config.stack_size)
        .max_io_events_per_tick(config.max_io_events_per_tick)
        .thread_name("novax-worker")
        .enable_all()
        .build()
        .expect("failed to build NovaX runtime")
}

pub fn build_default() -> Runtime {
    build(RuntimeConfig::default())
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build current-thread runtime");
    rt.block_on(future)
}

pub async fn run_with_timeout<F, T>(duration: Duration, future: F) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    match timeout(duration, future).await {
        Ok(result) => Ok(result),
        Err(_) => Err(TimeoutError),
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("operation timed out")]
pub struct TimeoutError;

pub fn spawn_task<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    spawn(future)
}
