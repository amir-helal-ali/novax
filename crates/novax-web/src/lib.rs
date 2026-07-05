//! NovaX Web UI
//!
//! Server-rendered HTML pages for authentication and admin dashboard.
//! All templates are static HTML strings with simple template substitution.

pub mod templates;
pub mod render;

pub use render::*;
