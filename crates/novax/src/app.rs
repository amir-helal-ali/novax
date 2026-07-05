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
    middleware::{from_fn_with_state, Next},
};
use novax_auth::{AuthService, AuthConfig, AuthUser, AuthSession, AuthError, extract_bearer_token};
use novax_network::{ServerConfig, serve, ServerError};
use novax_observability::system_health;
use novax_router::{RouterConfig, with_defaults};
use serde::{Serialize, Deserialize};
use sqlx::PgPool;
use tracing::{info, error};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::config::NovaXConfig;
use crate::db::{DatabaseConfig, create_pool, run_migrations};

/// Application shared state (DB-aware + Auth-aware)
#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
    pub config: Arc<RouterConfig>,
    pub db: Option<PgPool>,
    pub auth: Option<Arc<AuthService>>,
}

/// NovaX application
pub struct App {
    pub config: NovaXConfig,
    pub state: AppState,
    pub db_config: Option<DatabaseConfig>,
    pub auth_config: Option<AuthConfig>,
}

impl App {
    /// Create a new NovaX application with default configuration
    pub fn new() -> Self {
        let config = NovaXConfig::default();
        Self::with_config(config)
    }

    /// Create with configuration
    pub fn with_config(config: NovaXConfig) -> Self {
        let state = AppState {
            start_time: Instant::now(),
            config: Arc::new(config.router.clone()),
            db: None,
            auth: None,
        };
        Self { config, state, db_config: None, auth_config: None }
    }

    /// Configure database
    pub fn with_database(mut self, db_config: DatabaseConfig) -> Self {
        self.db_config = Some(db_config);
        self
    }

    /// Configure authentication
    pub fn with_auth(mut self, auth_config: AuthConfig) -> Self {
        self.auth_config = Some(auth_config);
        self
    }

    /// Initialize (connect DB, run migrations, init auth)
    pub async fn initialize(mut self) -> Result<Self, AppError> {
        if let Some(db_config) = &self.db_config {
            info!("Initializing database connection");
            let pool = create_pool(db_config).await
                .map_err(|e| AppError::Database(e.to_string()))?;

            // Run migrations
            if let Err(e) = run_migrations(&pool, "./migrations").await {
                error!("Migration failed: {}", e);
                return Err(AppError::Migration(e.to_string()));
            }

            self.state.db = Some(pool);

            // Initialize auth if configured
            if let Some(auth_config) = self.auth_config.take() {
                let auth = AuthService::new(auth_config);
                self.state.auth = Some(Arc::new(auth));
                info!("Authentication service initialized");
            }

            info!("Database initialized successfully");
        }
        Ok(self)
    }

    /// Run the application
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
            "NovaX application starting"
        );

        serve(router, server_config).await
    }

    /// Run with the configured address
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

/// Build the router with all routes
fn build_router(state: AppState) -> Router {
    let has_db = state.db.is_some();
    let has_auth = state.auth.is_some();

    let mut router: Router<AppState> = Router::new()
        // Core endpoints
        .route("/", get(dashboard))
        .route("/health", get(health_handler))
        .route("/api/health", get(api_health_handler))
        .route("/api/info", get(api_info_handler))
        .route("/api/version", get(api_version_handler))
        .route("/api/metrics", get(metrics_handler));

    if has_db {
        router = router
            // Users CRUD
            .route("/api/users", get(list_users))
            .route("/api/users/count", get(count_users))
            .route("/api/users/:id", get(get_user).patch(update_user).delete(delete_user))
            // Posts CRUD
            .route("/api/posts", get(list_posts).post(create_post))
            .route("/api/posts/count", get(count_posts))
            .route("/api/posts/:id", get(get_post).patch(update_post).delete(delete_post))
            .route("/api/users/:id/posts", get(list_user_posts));

        if has_auth {
            router = router
                // Auth endpoints (public)
                .route("/auth/register", post(auth_register))
                .route("/auth/login", post(auth_login))
                // Protected endpoints (require auth)
                .route("/auth/me", get(auth_me).layer(from_fn_with_state(state.clone(), require_auth)))
                .route("/auth/logout", post(auth_logout).layer(from_fn_with_state(state.clone(), require_auth)))
                .route("/auth/change-password", post(auth_change_password).layer(from_fn_with_state(state.clone(), require_auth)));
        }
    }

    let router = with_defaults(router, &state.config);
    router.with_state(state)
}

// ─── Auth Middleware ───

/// Auth context extracted by the require_auth middleware
#[derive(Clone, Debug)]
pub struct AuthContext {
    pub user: AuthUser,
}

/// Middleware: require a valid Bearer token
async fn require_auth(
    AxumState(state): AxumState<AppState>,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_header = req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::new(401, "Missing Authorization header"))?;

    let token = extract_bearer_token(auth_header)
        .ok_or_else(|| ApiError::new(401, "Invalid Authorization header format"))?;

    let auth = state.auth.as_ref().ok_or_else(|| ApiError::new(503, "Auth not configured"))?;
    let pool = state.db.as_ref().ok_or_else(|| ApiError::new(503, "Database not configured"))?;

    let user = auth.user_from_token(pool, token).await
        .map_err(|e| match e {
            AuthError::TokenExpired => ApiError::new(401, "Token expired"),
            AuthError::InvalidToken => ApiError::new(401, "Invalid token"),
            AuthError::UserNotFound => ApiError::new(401, "User not found"),
            other => ApiError::new(500, format!("auth error: {}", other)),
        })?;

    req.extensions_mut().insert(AuthContext { user });

    Ok(next.run(req).await)
}

// ─── Handlers ───

/// GET / — Dashboard HTML
async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// GET /health
async fn health_handler() -> Json<novax_observability::SystemHealth> {
    Json(system_health())
}

/// GET /api/health
async fn api_health_handler(AxumState(state): AxumState<AppState>) -> Json<serde_json::Value> {
    let db_status = if let Some(pool) = &state.db {
        match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        }
    } else {
        "disabled"
    };
    let auth_status = if state.auth.is_some() { "enabled" } else { "disabled" };
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.start_time.elapsed().as_secs(),
        "database": db_status,
        "auth": auth_status,
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
    auth_enabled: bool,
}

/// GET /api/info
async fn api_info_handler(AxumState(state): AxumState<AppState>) -> Json<AppInfo> {
    Json(AppInfo {
        name: "NovaX",
        version: env!("CARGO_PKG_VERSION"),
        description: "A next-generation full-stack web platform built entirely in Rust",
        homepage: "https://github.com/amir-helal-ali/novax",
        rust_version: "1.88+",
        features: vec![
            "Rust end-to-end",
            "HTTP/1.1 + HTTP/2",
            "Async runtime (tokio-based)",
            "Built-in observability",
            "PostgreSQL primary backend",
            "ORM with strongly-typed queries",
            "Migration engine with rollback",
            "Authentication (JWT + Argon2id)",
            "Authorization middleware",
            "Posts CRUD with FK relations",
            "Docker-ready",
            "Type-safe routing",
        ],
        database_enabled: state.db.is_some(),
        auth_enabled: state.auth.is_some(),
    })
}

/// GET /api/version
async fn api_version_handler() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// GET /api/metrics
async fn metrics_handler() -> String {
    novax_observability::REGISTRY.export_prometheus()
}

// ─── Auth Endpoints ───

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    email: String,
    name: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

/// POST /auth/register
async fn auth_register(
    AxumState(state): AxumState<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthUser>), ApiError> {
    let auth = state.auth.as_ref().ok_or_else(|| ApiError::new(503, "Auth not configured"))?;
    let pool = state.db.as_ref().ok_or_else(|| ApiError::new(503, "Database not configured"))?;

    let user = auth.register(pool, &body.email, &body.name, &body.password)
        .await
        .map_err(|e| match e {
            AuthError::UserExists => ApiError::new(409, "User already exists"),
            AuthError::WeakPassword => ApiError::new(400, "Password too weak (min 8 chars)"),
            other => ApiError::new(500, format!("auth error: {}", other)),
        })?;

    Ok((StatusCode::CREATED, Json(user)))
}

/// POST /auth/login
async fn auth_login(
    AxumState(state): AxumState<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    let auth = state.auth.as_ref().ok_or_else(|| ApiError::new(503, "Auth not configured"))?;
    let pool = state.db.as_ref().ok_or_else(|| ApiError::new(503, "Database not configured"))?;

    let session = auth.login(pool, &body.email, &body.password)
        .await
        .map_err(|e| match e {
            AuthError::InvalidCredentials => ApiError::new(401, "Invalid email or password"),
            other => ApiError::new(500, format!("auth error: {}", other)),
        })?;

    Ok(Json(session))
}

/// GET /auth/me (protected)
async fn auth_me(
    axum::Extension(ctx): axum::Extension<AuthContext>,
) -> Result<Json<AuthUser>, ApiError> {
    Ok(Json(ctx.user))
}

/// POST /auth/logout (protected)
async fn auth_logout(
    AxumState(state): AxumState<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
) -> Result<StatusCode, ApiError> {
    let auth = state.auth.as_ref().ok_or_else(|| ApiError::new(503, "Auth not configured"))?;
    let pool = state.db.as_ref().ok_or_else(|| ApiError::new(503, "Database not configured"))?;

    auth.logout(pool, ctx.user.id).await
        .map_err(|e| ApiError::new(500, format!("logout error: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /auth/change-password (protected)
async fn auth_change_password(
    AxumState(state): AxumState<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<StatusCode, ApiError> {
    let auth = state.auth.as_ref().ok_or_else(|| ApiError::new(503, "Auth not configured"))?;
    let pool = state.db.as_ref().ok_or_else(|| ApiError::new(503, "Database not configured"))?;

    auth.change_password(pool, ctx.user.id, &body.current_password, &body.new_password)
        .await
        .map_err(|e| match e {
            AuthError::InvalidCredentials => ApiError::new(401, "Current password is incorrect"),
            AuthError::WeakPassword => ApiError::new(400, "Password too weak (min 8 chars)"),
            other => ApiError::new(500, format!("auth error: {}", other)),
        })?;

    Ok(StatusCode::OK)
}

// ─── Users CRUD ───

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct PaginationParams {
    page: Option<u32>,
    per_page: Option<u32>,
}

fn db_required(state: &AppState) -> Result<&PgPool, ApiError> {
    state.db.as_ref().ok_or_else(|| ApiError::new(503, "Database not configured"))
}

/// GET /api/users
async fn list_users(
    AxumState(state): AxumState<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = db_required(&state)?;
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    let users: Vec<User> = sqlx::query_as(
        "SELECT id, email, name, password_hash, bio, avatar_url, is_active, created_at, updated_at
         FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2"
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

/// GET /api/users/count
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

/// GET /api/users/:id
async fn get_user(
    AxumState(state): AxumState<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, ApiError> {
    let pool = db_required(&state)?;
    let user: User = sqlx::query_as(
        "SELECT id, email, name, password_hash, bio, avatar_url, is_active, created_at, updated_at FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?
    .ok_or_else(|| ApiError::new(404, "User not found"))?;
    Ok(Json(user))
}

#[derive(Debug, Deserialize)]
struct UpdateUserRequest {
    email: Option<String>,
    name: Option<String>,
    bio: Option<String>,
    avatar_url: Option<String>,
    is_active: Option<bool>,
}

/// PATCH /api/users/:id
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
           RETURNING id, email, name, password_hash, bio, avatar_url, is_active, created_at, updated_at"#,
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

/// DELETE /api/users/:id
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

// ─── Posts CRUD ───

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct Post {
    pub id: Uuid,
    pub author_id: Uuid,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub is_published: bool,
    pub view_count: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreatePostRequest {
    pub author_id: Uuid,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub is_published: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdatePostRequest {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub body: Option<String>,
    pub is_published: Option<bool>,
}

/// GET /api/posts
async fn list_posts(
    AxumState(state): AxumState<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = db_required(&state)?;
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    let posts: Vec<Post> = sqlx::query_as(
        "SELECT id, author_id, title, slug, body, is_published, view_count, published_at, created_at, updated_at
         FROM posts ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    )
    .bind(per_page as i64)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts")
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "items": posts,
        "total": total.0,
        "page": page,
        "per_page": per_page,
        "total_pages": ((total.0 as u32) + per_page - 1) / per_page,
    })))
}

/// GET /api/posts/count
async fn count_posts(
    AxumState(state): AxumState<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = db_required(&state)?;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts")
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;
    Ok(Json(serde_json::json!({"count": count.0})))
}

/// POST /api/posts
async fn create_post(
    AxumState(state): AxumState<AppState>,
    Json(body): Json<CreatePostRequest>,
) -> Result<(StatusCode, Json<Post>), ApiError> {
    let pool = db_required(&state)?;

    if body.title.is_empty() || body.slug.is_empty() || body.body.is_empty() {
        return Err(ApiError::new(400, "title, slug, and body are required"));
    }

    let is_published = body.is_published.unwrap_or(false);
    let published_at = if is_published { Some(Utc::now()) } else { None };

    let post: Post = sqlx::query_as(
        r#"INSERT INTO posts (author_id, title, slug, body, is_published, published_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, author_id, title, slug, body, is_published, view_count, published_at, created_at, updated_at"#,
    )
    .bind(body.author_id)
    .bind(&body.title)
    .bind(&body.slug)
    .bind(&body.body)
    .bind(is_published)
    .bind(published_at)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return ApiError::new(409, "Slug already exists");
            }
            if db_err.is_foreign_key_violation() {
                return ApiError::new(400, "Author not found");
            }
        }
        ApiError::new(500, format!("db error: {}", e))
    })?;

    Ok((StatusCode::CREATED, Json(post)))
}

/// GET /api/posts/:id
async fn get_post(
    AxumState(state): AxumState<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Post>, ApiError> {
    let pool = db_required(&state)?;
    let post: Post = sqlx::query_as(
        "SELECT id, author_id, title, slug, body, is_published, view_count, published_at, created_at, updated_at FROM posts WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?
    .ok_or_else(|| ApiError::new(404, "Post not found"))?;

    // Increment view count (fire and forget — async update)
    let _ = sqlx::query("UPDATE posts SET view_count = view_count + 1 WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await;

    Ok(Json(post))
}

/// PATCH /api/posts/:id
async fn update_post(
    AxumState(state): AxumState<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePostRequest>,
) -> Result<Json<Post>, ApiError> {
    let pool = db_required(&state)?;

    // If publishing for the first time, set published_at
    let set_published_at = if body.is_published == Some(true) {
        "CASE WHEN published_at IS NULL THEN NOW() ELSE published_at END"
    } else {
        "published_at"
    };

    let query = format!(
        r#"UPDATE posts SET
            title = COALESCE($2, title),
            slug = COALESCE($3, slug),
            body = COALESCE($4, body),
            is_published = COALESCE($5, is_published),
            published_at = CASE WHEN $5 = true AND published_at IS NULL THEN NOW() ELSE published_at END,
            updated_at = NOW()
           WHERE id = $1
           RETURNING id, author_id, title, slug, body, is_published, view_count, published_at, created_at, updated_at"#,
    );

    let post: Post = sqlx::query_as(&query)
        .bind(id)
        .bind(&body.title)
        .bind(&body.slug)
        .bind(&body.body)
        .bind(body.is_published)
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?
        .ok_or_else(|| ApiError::new(404, "Post not found"))?;

    let _ = set_published_at;  // suppress unused warning
    Ok(Json(post))
}

/// DELETE /api/posts/:id
async fn delete_post(
    AxumState(state): AxumState<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let pool = db_required(&state)?;
    let result = sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;
    if result.rows_affected() == 0 {
        Err(ApiError::new(404, "Post not found"))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// GET /api/users/:id/posts — list posts by a specific user
async fn list_user_posts(
    AxumState(state): AxumState<AppState>,
    Path(user_id): Path<Uuid>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = db_required(&state)?;
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let offset = ((page - 1) * per_page) as i64;

    let posts: Vec<Post> = sqlx::query_as(
        "SELECT id, author_id, title, slug, body, is_published, view_count, published_at, created_at, updated_at
         FROM posts WHERE author_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
    )
    .bind(user_id)
    .bind(per_page as i64)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts WHERE author_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::new(500, format!("db error: {}", e)))?;

    Ok(Json(serde_json::json!({
        "user_id": user_id,
        "items": posts,
        "total": total.0,
        "page": page,
        "per_page": per_page,
        "total_pages": ((total.0 as u32) + per_page - 1) / per_page,
    })))
}

// ─── Error Types ───

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

/// Application-level error
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),
    #[error("migration error: {0}")]
    Migration(String),
}

const DASHBOARD_HTML: &str = include_str!("../../../static/index.html");
