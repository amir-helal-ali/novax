//! NovaX prelude — re-exports most commonly used types

pub use crate::{
    app::App,
    config::{Environment, NovaXConfig, ServerConfig},
    version,
};

pub use novax_macros::{entity, main, route};
pub use novax_network;
pub use novax_observability::{self, init_logging, system_health, HealthStatus, SystemHealth};
pub use novax_router::{
    self, AppState, Json, Method, Response, Router, RouterConfig, StatusCode,
    error_response, get, json_response, post,
};
pub use novax_runtime::{self, block_on, build, build_default, spawn, spawn_task};
pub use novax_storage::{self, BackendKind, Storage, StorageConfig, StorageError};

pub use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
    routing::{delete, patch, put},
};
