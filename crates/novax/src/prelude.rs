//! NovaX prelude — re-exports most commonly used types

pub use crate::{
    app::App,
    config::{Environment, NovaXConfig, ServerConfig},
    db::{DatabaseConfig, create_pool, run_migrations},
    version,
};

pub use novax_auth::{
    self, AuthConfig, AuthError, AuthService, AuthSession, AuthUser, Claims,
    EmailVerificationToken, PasswordResetToken,
    OAuthConfig, OAuthProvider, OAuthProviderConfig, OAuthUserInfo,
    build_auth_url, extract_bearer_token, generate_state,
};
pub use novax_macros::{entity, main, route};
pub use novax_mail::{self, MailConfig, MailError, MailService};
pub use novax_migrate::{self, Migration, MigrationError, MigrationReport, MigrationRunner};
pub use novax_network;
pub use novax_observability::{self, init_logging, system_health, HealthStatus, SystemHealth};
pub use novax_orm::{self, Entity, OrmError, PaginatedResult, Pagination, Repository};
pub use novax_rate_limit::{self, RateLimiter, RateLimitConfig};
pub use novax_router::{
    self, AppState, Json, Method, Response, Router, RouterConfig, StatusCode,
    error_response, get, json_response, post,
};
pub use novax_runtime::{self, block_on, build, build_default, spawn, spawn_task};
pub use novax_storage::{self, BackendKind, Storage, StorageConfig, StorageError};
pub use novax_web::render;

pub use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
    routing::{delete, patch, put},
};

pub use sqlx::{self, PgPool};
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc};
