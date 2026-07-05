//! NovaX
//!
//! A next-generation full-stack web platform built entirely in Rust.
//!
//! ## Quick Start
//! ```rust,no_run
//! use novax::prelude::*;
//!
//! #[novax::main]
//! async fn main() {
//!     let app = novax::app::App::new()
//!         .with_route("GET", "/", || async { "Hello, NovaX!" });
//!
//!     app.serve("0.0.0.0:3000").await.unwrap();
//! }
//! ```

pub use novax_macros::{entity, main, route};
pub use novax_migrate as migrate;
pub use novax_network as network;
pub use novax_observability as observability;
pub use novax_orm as orm;
pub use novax_router as router;
pub use novax_runtime as runtime;
pub use novax_storage as storage;

pub mod app;
pub mod config;
pub mod db;
pub mod prelude;

/// Get the NovaX version
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
