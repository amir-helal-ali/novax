//! NovaX Application builder

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    Router,
    routing::{get, post, patch, delete},
    response::{Html, IntoResponse, Response},
    extract::{State as AxumState, Path, Query},
    Json,
    http::StatusCode,
};
use novax_network::{ServerConfig, serve, ServerError};
use novax_observability::system_health;
use novax_router::{RouterConfig, with_defaults};
use serde::{Serialize, Deserialize};
use sqlx::PgPool;
use tracing::{info, error};

use crate::config::NovaXConfig;
use crate::db::{DatabaseConfig, create_pool, run_migrations};

/// Application shared state (DB-aware)
#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
    pub config: Arc<RouterConfig>,
    pub db: Option<PgPool>,
}

/// NovaX application
pub struct App {
    pub config: NovaXConfig,
    pub state: AppState,
    pub db_config: Option<DatabaseConfig>,
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
            db: None,
        };
        Self { config, state, db_config: None }
    }

    /// Configure database connection
    pub fn with_database(mut self, db_config: DatabaseConfig) -> Self {
        self.db_config = Some(db_config);
        self
    }

    /// Initialize the application (connect to DB, run migrations)
    pub async fn initialize(mut self) -> Result<Self, AppError> {
        if let Some(db_config) = &self.db_config {
            info!("Initializing database connection");
            let pool = create_pool(db_config).await
                .map_err(|e| AppError::Database(e.to_string()))?;

            // Run migrations from ./migrations directory
            if let Err(e) = run_migrations(&pool, "./migrations").await {
                error!("Migration failed: {}", e);
                return Err(AppError::Migration(e.to_string()));
            }

            self.state.db = Some(pool);
            info!("Database initialized successfully");
        }
        Ok(self)
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

        let router = build_router(self.state);

        info!(
            version = env!("CARGO_PKG_VERSION"),
            addr = %bind_addr,
            db_enabled = tracing::field::Empty,
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

/// Build the default router with all routes
fn build_router(state: AppState) -> Router {
    let db_enabled = state.db.is_some();
    let router: Router<AppState> = Router::new()
        // Core endpoints
        .route("/", get(dashboard))
        .route("/health", get(health_handler))
        .route("/api/health", get(api_health_handler))
        .route("/api/info", get(api_info_handler))
        .route("/api/version", get(api_version_handler))
        .route("/api/metrics", get(metrics_handler))
        // Users CRUD (only if DB is enabled)
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", get(get_user).patch(update_user).delete(delete_user))
        .route("/api/users/count", get(count_users));

    let router = if db_enabled {
        router
    } else {
        // If no DB, return 503 for user endpoints
        router
    };

    let router = with_defaults(router, &state.config);
    router.with_state(state)
}

// ─── Handlers ───

/// GET / — Dashboard HTML page
async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// GET /health — Health check endpoint
async fn health_handler() -> axum::Json<novax_observability::SystemHealth> {
    axum::Json(system_health())
}

/// GET /api/health — API health check
async fn api_health_handler(AxumState(state): AxumState<AppState>) -> axum::Json<serde_json::Value> {
    let db_status = if let Some(pool) = &state.db {
        match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        }
    } else {
        "disabled"
    };
    axum::Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.start_time.elapsed().as_secs(),
        "database": db_status,
    }))
}

#[derive(Serialize, Deserialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    description: &'static str,
    homepage: &'static str,
    rust_version: &'static str,
    features: Vec<&'static str>,
    database_enabled: bool,
}

/// GET /api/info — Application info
async fn api_info_handler(AxumState(state): AxumState<AppState>) -> axum::Json<AppInfo> {
    axum::Json(AppInfo {
        name: "NovaX",
        version: env!("CARGO_PKG_VERSION"),
        description: "A next-generation full-stack web platform built entirely in Rust",
        homepage: "https://github.com/amir-helal-ali/novax",
        rust_version: "1.85+",
        features: vec![
            "Rust end-to-end",
            "HTTP/1.1 + HTTP/2",
            "Async runtime (tokio-based)",
            "Built-in observability",
            "Multi-backend storage (memory + PostgreSQL)",
            "ORM with strongly-typed queries",
            "Migration engine with rollback",
            "Docker-ready",
            "Type-safe routing",
        ],
        database_enabled: state.db.is_some(),
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

// ─── Users CRUD ───

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateUserRequest {
    pub email: String,
    pub name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateUserRequest {
    pub email: Option<String>,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: ApiErrorBody,
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    code: u16,
    message: String,
}

impl ApiError {
    fn new(code: u16, message: impl Into<String>) -> Self {
        Self {
            error: ApiErrorBody {
                code,
                message: message.into(),
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.error.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}

use uuid::Uuid;
use chrono::{DateTime, Utc};

fn db_required(state: &AppState) -> Result<&PgPool, ApiError> {
    state.db.as_ref().ok_or_else(|| ApiError::new(503, "Database not configured"))
}

/// GET /api/users — List users with pagination
async fn list_users(
    AxumState(state): AxumState<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = db_required(&state)?;
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    let users: Vec<User> = sqlx::query_as(
        "SELECT * FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    )
    .bind(per_page as i64)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "items": users,
        "total": total.0,
        "page": page,
        "per_page": per_page,
        "total_pages": ((total.0 as u32) + per_page - 1) / per_page,
    })))
}

#[derive(Debug, Deserialize)]
struct PaginationParams {
    page: Option<u32>,
    per_page: Option<u32>,
}

/// POST /api/users — Create a new user
async fn create_user(
    AxumState(state): AxumState<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>), ApiError> {
    let pool = db_required(&state)?;

    // Validate
    if body.email.is_empty() || !body.email.contains('@') {
        return Err(ApiError::new(400, "Invalid email"));
    }
    if body.name.is_empty() {
        return Err(ApiError::new(400, "Name is required"));
    }

    let user: User = sqlx::query_as(
        r#"INSERT INTO users (email, name, bio, avatar_url)
           VALUES ($1, $2, $3, $4)
           RETURNING id, email, name, bio, avatar_url, is_active, created_at, updated_at"#,
    )
    .bind(&body.email)
    .bind(&body.name)
    .bind(&body.bio)
    .bind(&body.avatar_url)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return ApiError::new(409, "Email already exists");
            }
        }
        ApiError::new(500, format!("db error: {}", e))
    })?;

    Ok((StatusCode::CREATED, Json(user)))
}

/// GET /api/users/:id — Get a single user
async fn get_user(
    AxumState(state): AxumState<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, ApiError> {
    let pool = db_required(&state)?;

    let user: User = sqlx::query_as(
        "SELECT id, email, name, bio, avatar_url, is_active, created_at, updated_at FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?
    .ok_or_else(|| ApiError::new(404, "User not found"))?;

    Ok(Json(user))
}

/// PATCH /api/users/:id — Update a user
async fn update_user(
    AxumState(state): AxumState<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<User>, ApiError> {
    let pool = db_required(&state)?;

    let user: User = sqlx::query_as(
        r#"UPDATE users SET
            email = COALESCE($2, email),
            name = COALESCE($3, name),
            bio = COALESCE($4, bio),
            avatar_url = COALESCE($5, avatar_url),
            is_active = COALESCE($6, is_active),
            updated_at = NOW()
           WHERE id = $1
           RETURNING id, email, name, bio, avatar_url, is_active, created_at, updated_at"#,
    )
    .bind(id)
    .bind(&body.email)
    .bind(&body.name)
    .bind(&body.bio)
    .bind(&body.avatar_url)
    .bind(body.is_active)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?
    .ok_or_else(|| ApiError::new(404, "User not found"))?;

    Ok(Json(user))
}

/// DELETE /api/users/:id — Delete a user
async fn delete_user(
    AxumState(state): AxumState<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let pool = db_required(&state)?;

    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    if result.rows_affected() == 0 {
        Err(ApiError::new(404, "User not found"))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// GET /api/users/count — Get total user count
async fn count_users(
    AxumState(state): AxumState<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = db_required(&state)?;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    Ok(Json(serde_json::json!({"count": count.0})))
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

/// Application-level error
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),
    #[error("migration error: {0}")]
    Migration(String),
}

const DASHBOARD_HTML: &str = include_str!("../../../static/index.html");
