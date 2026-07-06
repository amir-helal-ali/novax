//! NovaX Application builder (v0.4)
//!
//! Full-featured application with:
//! - Authentication (JWT + Argon2 + email verification + password reset)
//! - OAuth2 (Google + GitHub)
//! - Rate limiting (configurable)
//! - File uploads (avatars)
//! - Admin dashboard (users management + settings UI)
//! - Server-rendered HTML auth pages

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    Router,
    routing::{get, post},
    response::{Html, IntoResponse, Response, Redirect},
    extract::{State, Path, Query, Multipart, Form},
    Json,
    http::StatusCode,
    middleware::{from_fn_with_state, Next},
};
use novax_auth::{
    AuthService, AuthConfig, AuthUser, AuthError, extract_bearer_token,
    OAuthConfig, OAuthProvider, build_auth_url, generate_state,
};
use novax_mail::{MailConfig, MailService};
use novax_network::{ServerConfig, serve, ServerError};
use novax_rate_limit::{RateLimiter, RateLimitConfig, spawn_cleanup_task};
use novax_router::{RouterConfig, with_defaults};
use novax_seo::{generate_robots_txt, generate_sitemap, generate_manifest, default_sitemap, SeoConfig};
use novax_web::render::*;
use novax_core::{ProjectConfig, EntityConfig, FieldConfig, FieldType, ThemeConfig};
use novax_compiler::{build_project, GeneratedFile, generate_openapi_spec, generate_swagger_ui};
use serde::{Serialize, Deserialize};
use sqlx::PgPool;
use sqlx::Row;
use tracing::{info, error, warn, debug};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::config::NovaXConfig;
use crate::db::{DatabaseConfig, create_pool, run_migrations};

/// Application shared state
#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
    pub config: Arc<RouterConfig>,
    pub db: Option<PgPool>,
    pub auth: Option<Arc<AuthService>>,
    pub rate_limiter: Option<RateLimiter>,
    pub oauth_config: Option<OAuthConfig>,
    pub mail: Option<Arc<MailService>>,
    pub http_client: Option<reqwest::Client>,
    pub dev_mode: bool,
}

/// NovaX application
pub struct App {
    pub config: NovaXConfig,
    pub state: AppState,
    pub db_config: Option<DatabaseConfig>,
    pub auth_config: Option<AuthConfig>,
    pub rate_limit_config: Option<RateLimitConfig>,
    pub oauth_config: Option<OAuthConfig>,
    pub mail_config: Option<MailConfig>,
    pub dev_mode: bool,
}

impl App {
    pub fn new() -> Self {
        let config = NovaXConfig::default();
        Self::with_config(config)
    }

    pub fn with_config(config: NovaXConfig) -> Self {
        let state = AppState {
            start_time: Instant::now(),
            config: Arc::new(config.router.clone()),
            db: None,
            auth: None,
            rate_limiter: None,
            oauth_config: None,
            mail: None,
            http_client: None,
            dev_mode: false,
        };
        Self {
            config,
            state,
            db_config: None,
            auth_config: None,
            rate_limit_config: None,
            oauth_config: None,
            mail_config: None,
            dev_mode: false,
        }
    }

    pub fn with_database(mut self, db_config: DatabaseConfig) -> Self {
        self.db_config = Some(db_config);
        self
    }

    pub fn with_auth(mut self, auth_config: AuthConfig) -> Self {
        self.auth_config = Some(auth_config);
        self
    }

    pub fn with_rate_limiting(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit_config = Some(config);
        self
    }

    pub fn with_oauth(mut self, config: OAuthConfig) -> Self {
        self.oauth_config = Some(config);
        self
    }

    pub fn with_mail(mut self, config: MailConfig) -> Self {
        self.mail_config = Some(config);
        self
    }

    pub fn dev_mode(mut self) -> Self {
        self.dev_mode = true;
        self
    }

    pub async fn initialize(mut self) -> Result<Self, AppError> {
        if let Some(db_config) = &self.db_config {
            info!("Initializing database connection");
            let pool = create_pool(db_config).await
                .map_err(|e| AppError::Database(e.to_string()))?;

            if let Err(e) = run_migrations(&pool, "./migrations").await {
                error!("Migration failed: {}", e);
                return Err(AppError::Migration(e.to_string()));
            }

            self.state.db = Some(pool.clone());

            if let Some(auth_config) = self.auth_config.take() {
                let auth = AuthService::new(auth_config);
                self.state.auth = Some(Arc::new(auth.clone()));
                info!("Authentication service initialized");

                // Seed default admin user
                match auth.seed_admin_user(&pool).await {
                    Ok(novax_auth::SeedResult::Created { email, password }) => {
                        info!(
                            email = %email,
                            "✅ Default admin user created — login with the configured ADMIN_PASSWORD"
                        );
                        if password == "admin12345" {
                            warn!("⚠️  Using default admin password — set ADMIN_PASSWORD env var!");
                        }
                    }
                    Ok(novax_auth::SeedResult::AlreadyExists) => {
                        debug!("Admin user already exists — skipping seed");
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to seed admin user — continuing anyway");
                    }
                }
            }

            if let Some(rl_config) = self.rate_limit_config.take() {
                let limiter = RateLimiter::new(rl_config);
                spawn_cleanup_task(limiter.clone());
                self.state.rate_limiter = Some(limiter);
                info!("Rate limiting initialized");
            }

            // Initialize mail service
            let mail_config = self.mail_config.take().unwrap_or_default();
            let mail = MailService::new(mail_config);
            self.state.mail = Some(Arc::new(mail));
            info!("Mail service initialized");

            // Initialize HTTP client (for OAuth)
            self.state.http_client = Some(reqwest::Client::new());

            self.state.oauth_config = self.oauth_config.take();
            self.state.dev_mode = self.dev_mode;

            info!("Database + Auth + Rate Limiting + Mail initialized successfully");
        }
        Ok(self)
    }

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
    let has_rate_limit = state.rate_limiter.is_some();
    let has_oauth = state.oauth_config.as_ref().is_some_and(|c| c.any_enabled());

    let mut router: Router<AppState> = Router::new()
        // Core API
        .route("/", get(dashboard_root))
        .route("/api/health", get(api_health_handler))
        // SEO endpoints
        .route("/sitemap.xml", get(sitemap_handler))
        .route("/robots.txt", get(robots_handler))
        .route("/manifest.json", get(manifest_handler))
        .route("/api/info", get(api_info_handler))
        .route("/api/version", get(api_version_handler))
        .route("/api/metrics", get(metrics_handler));

    if has_db && has_auth {
        // Auth UI pages (HTML)
        router = router
            .route("/auth/login", get(login_page_handler).post(login_form_handler))
            .route("/auth/register", get(register_page_handler).post(register_form_handler))
            .route("/auth/logout", post(logout_handler))
            .route("/auth/forgot-password", get(forgot_password_page_handler).post(forgot_password_form_handler))
            .route("/auth/reset-password", get(reset_password_page_handler).post(reset_password_form_handler))
            .route("/auth/verify-email", get(verify_email_handler))
            .route("/auth/change-password", post(change_password_form_handler).layer(from_fn_with_state(state.clone(), require_auth)));

        // OAuth routes
        if has_oauth {
            router = router
                .route("/auth/oauth/google", get(oauth_google_handler))
                .route("/auth/oauth/github", get(oauth_github_handler))
                .route("/auth/oauth/:provider/callback", get(oauth_callback_handler));
        }

        // API auth endpoints (JSON)
        router = router
            .route("/api/auth/me", get(api_auth_me).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/api/auth/logout", post(api_auth_logout).layer(from_fn_with_state(state.clone(), require_auth)));

        // Admin dashboard (protected + admin only)
        router = router
            .route("/admin", get(admin_dashboard_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/users", get(admin_users_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/users/:id", get(admin_user_detail_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/users/:id/toggle-active", post(admin_toggle_active_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/users/:id/toggle-admin", post(admin_toggle_admin_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/settings", get(admin_settings_handler).post(admin_settings_form_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/delete-user/:id", post(admin_delete_user_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/users/:id/edit", get(admin_user_edit_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/users/:id/update", post(admin_user_update_handler).layer(from_fn_with_state(state.clone(), require_auth)));

        // Profile page (for regular users — shows own profile)
        router = router
            .route("/profile", get(profile_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/profile/update", post(profile_update_handler).layer(from_fn_with_state(state.clone(), require_auth)));

        // Avatar upload (protected)
        router = router
            .route("/api/users/avatar", post(upload_avatar_handler).layer(from_fn_with_state(state.clone(), require_auth)));

        // ─── Novax Engine: Project Management (admin only) ───
        router = router
            .route("/admin/projects", get(admin_projects_handler).post(admin_create_project_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id", get(admin_project_detail_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/entities", post(admin_add_entity_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/export", get(admin_export_project_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/preview", get(admin_preview_project_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            // Entity field editor
            .route("/admin/projects/:id/entities/:entity_id/fields", get(admin_entity_fields_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/entities/:entity_id/fields/add", post(admin_add_field_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/entities/:entity_id/fields/:field_name/delete", post(admin_delete_field_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            // Theme editor
            .route("/admin/projects/:id/theme", get(admin_theme_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/theme/update", post(admin_theme_update_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            // Download project as tar.gz
            .route("/admin/projects/:id/download", get(admin_download_project_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            // Delete entity
            .route("/admin/projects/:id/entities/:entity_id/delete", post(admin_delete_entity_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            // Twin-Links: API Inspector (Swagger UI + OpenAPI JSON)
            .route("/admin/projects/:id/api-docs", get(admin_swagger_ui_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/api-spec.json", get(admin_openapi_spec_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            // Project settings (edit/delete project)
            .route("/admin/projects/:id/settings", get(admin_project_settings_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/delete", post(admin_delete_project_handler).layer(from_fn_with_state(state.clone(), require_auth)))
            .route("/admin/projects/:id/update", post(admin_update_project_handler).layer(from_fn_with_state(state.clone(), require_auth)));
    }

    // Apply rate limiting globally
    if has_rate_limit {
        if let Some(limiter) = &state.rate_limiter {
            let limiter_clone = limiter.clone();
            router = router.layer(axum::middleware::from_fn(move |req, next| {
                let limiter = limiter_clone.clone();
                async move { rate_limit_middleware_inner(limiter, req, next).await }
            }));
        }
    }

    let router = with_defaults(router, &state.config);
    router.with_state(state)
}

/// Rate limit middleware inner
async fn rate_limit_middleware_inner(
    limiter: RateLimiter,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    use novax_rate_limit::{extract_client_ip, RateLimitResult};
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
            response.headers_mut().insert("x-ratelimit-limit", limit.to_string().parse().unwrap());
            response.headers_mut().insert("x-ratelimit-remaining", remaining.to_string().parse().unwrap());
            response.headers_mut().insert("retry-after", retry_after_seconds.to_string().parse().unwrap());
            response
        }
    }
}

// ─── Auth Middleware ───

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub user: AuthUser,
}

async fn require_auth(
    State(state): State<AppState>,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Redirect> {
    // First try Authorization Bearer header
    let token = req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(extract_bearer_token);

    // Fallback: cookie-based session
    let token = if let Some(t) = token {
        Some(t.to_string())
    } else {
        req.headers()
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies.split(';')
                    .map(|c| c.trim())
                    .find_map(|c| c.strip_prefix("novax_token=").map(|t| t.to_string()))
            })
    };

    let Some(token) = token else {
        // Redirect to login if accessing via browser, 401 for API
        let path = req.uri().path();
        if path.starts_with("/api/") {
            return Ok((StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": {"code": 401, "message": "Authentication required"}}))).into_response());
        }
        return Err(Redirect::to("/auth/login"));
    };

    let auth = state.auth.as_ref().ok_or_else(|| Redirect::to("/auth/login"))?;
    let pool = state.db.as_ref().ok_or_else(|| Redirect::to("/auth/login"))?;

    let user = auth.user_from_token(pool, &token).await
        .map_err(|_| Redirect::to("/auth/login"))?;

    req.extensions_mut().insert(AuthContext { user });
    Ok(next.run(req).await)
}

/// Admin-only middleware (used inside handlers via extension check)
fn require_admin(ctx: &AuthContext) -> Result<(), Response> {
    if ctx.user.is_admin {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, Html("<h1>403 — Forbidden</h1><p>Admin access required.</p>")).into_response())
    }
}

// ─── Handlers: Core API ───

async fn dashboard_root(State(state): State<AppState>) -> Response {
    if state.db.is_some() && state.auth.is_some() {
        // إذا كان المستخدم مسجل دخول (عنده cookie)، وجهه لـ /admin
        // وإلا وجهه لصفحة الهبوط الجميلة
        Html(novax_web::landing_page()).into_response()
    } else {
        Html(novax_web::landing_page()).into_response()
    }
}

async fn api_health_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db_status = if let Some(pool) = &state.db {
        match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        }
    } else {
        "disabled"
    };
    let auth_status = if state.auth.is_some() { "enabled" } else { "disabled" };
    let rl_status = if state.rate_limiter.is_some() { "enabled" } else { "disabled" };
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.start_time.elapsed().as_secs(),
        "database": db_status,
        "auth": auth_status,
        "rate_limiting": rl_status,
        "oauth": if state.oauth_config.as_ref().is_some_and(|c| c.any_enabled()) { "enabled" } else { "disabled" },
    }))
}

#[derive(Serialize, Deserialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    description: &'static str,
    features: Vec<&'static str>,
    database_enabled: bool,
    auth_enabled: bool,
    rate_limiting_enabled: bool,
    oauth_enabled: bool,
}

async fn api_info_handler(State(state): State<AppState>) -> Json<AppInfo> {
    Json(AppInfo {
        name: "NovaX",
        version: env!("CARGO_PKG_VERSION"),
        description: "A next-generation full-stack web platform built entirely in Rust",
        features: vec![
            "Rust end-to-end", "HTTP/1.1 + HTTP/2", "Async runtime (tokio)",
            "PostgreSQL primary backend", "Authentication (JWT + Argon2id)",
            "Email verification + password reset", "OAuth2 (Google + GitHub)",
            "Rate limiting (configurable)", "Avatar uploads",
            "Admin dashboard", "Migration engine",
        ],
        database_enabled: state.db.is_some(),
        auth_enabled: state.auth.is_some(),
        rate_limiting_enabled: state.rate_limiter.is_some(),
        oauth_enabled: state.oauth_config.as_ref().is_some_and(|c| c.any_enabled()),
    })
}

async fn api_version_handler() -> &'static str { env!("CARGO_PKG_VERSION") }

async fn metrics_handler() -> String {
    novax_observability::REGISTRY.export_prometheus()
}

// ─── SEO Handlers ───

/// GET /sitemap.xml — XML sitemap for search engines
async fn sitemap_handler() -> impl IntoResponse {
    let config = SeoConfig::default();
    let urls = default_sitemap(&config.site_url);
    let xml = generate_sitemap(&config.site_url, &urls);
    (
        [(axum::http::header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        xml,
    )
}

/// GET /robots.txt — robots.txt for crawlers
async fn robots_handler() -> impl IntoResponse {
    let config = SeoConfig::default();
    let txt = generate_robots_txt(&config.site_url);
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        txt,
    )
}

/// GET /manifest.json — PWA manifest
async fn manifest_handler() -> impl IntoResponse {
    let config = SeoConfig::default();
    let manifest = generate_manifest(&config);
    Json(manifest)
}

// ─── Handlers: Auth UI Pages ───

async fn login_page_handler(State(state): State<AppState>) -> Html<String> {
    let oauth = state.oauth_config.as_ref().is_some_and(|c| c.any_enabled());
    Html(login_page(None, oauth))
}

async fn register_page_handler(State(state): State<AppState>) -> Html<String> {
    let oauth = state.oauth_config.as_ref().is_some_and(|c| c.any_enabled());
    Html(register_page(None, oauth))
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

async fn login_form_handler(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Response {
    let auth = match state.auth.as_ref() {
        Some(a) => a,
        None => return Html(login_page(Some("Auth not configured"), false)).into_response(),
    };
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return Html(login_page(Some("Database not configured"), false)).into_response(),
    };

    match auth.login(pool, &form.email, &form.password).await {
        Ok(session) => {
            // Set cookie + redirect to admin
            let cookie = format!(
                "novax_token={}; Path=/; HttpOnly; Max-Age=3600; SameSite=Lax",
                session.token
            );
            let mut response = Redirect::to("/admin").into_response();
            response.headers_mut().insert(
                axum::http::header::SET_COOKIE,
                cookie.parse().unwrap(),
            );
            response
        }
        Err(_e) => {
            let msg = "بريد إلكتروني أو كلمة مرور غير صحيحة";
            Html(login_page(Some(msg), state.oauth_config.as_ref().is_some_and(|c| c.any_enabled()))).into_response()
        }
    }
}

#[derive(Deserialize)]
struct RegisterForm {
    email: String,
    name: String,
    password: String,
}

async fn register_form_handler(
    State(state): State<AppState>,
    Form(form): Form<RegisterForm>,
) -> Response {
    let auth = match state.auth.as_ref() {
        Some(a) => a,
        None => return Html(register_page(Some("Auth not configured"), false)).into_response(),
    };
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return Html(register_page(Some("Database not configured"), false)).into_response(),
    };

    match auth.register(pool, &form.email, &form.name, &form.password).await {
        Ok(user) => {
            // Generate email verification token
            let token_result = auth.create_email_verification_token(pool, user.id).await;
            let dev_token = if state.dev_mode {
                token_result.as_ref().ok().cloned()
            } else {
                // Send verification email via SMTP
                if let (Ok(token), Some(mail)) = (&token_result, state.mail.as_ref()) {
                    if let Err(e) = mail.send_verification_email(&user.email, &user.name, token).await {
                        warn!(error = %e, "Failed to send verification email");
                    }
                }
                None
            };
            Html(verification_notice_page(&user.email, dev_token.as_deref())).into_response()
        }
        Err(e) => {
            let msg = match e {
                AuthError::UserExists => "هذا البريد مسجل بالفعل",
                AuthError::WeakPassword => "كلمة المرور ضعيفة (8 أحرف على الأقل)",
                _ => "حدث خطأ، حاول مرة أخرى",
            };
            Html(register_page(Some(msg), state.oauth_config.as_ref().is_some_and(|c| c.any_enabled()))).into_response()
        }
    }
}

async fn logout_handler(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
) -> Response {
    if let (Some(auth), Some(pool)) = (state.auth.as_ref(), state.db.as_ref()) {
        let _ = auth.logout(pool, ctx.user.id).await;
    }
    // Clear cookie + redirect to login
    let cookie = "novax_token=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax";
    let mut response = Redirect::to("/auth/login").into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        cookie.parse().unwrap(),
    );
    response
}

async fn forgot_password_page_handler() -> Html<String> {
    Html(forgot_password_page(None, false))
}

#[derive(Deserialize)]
struct ForgotPasswordForm {
    email: String,
}

async fn forgot_password_form_handler(
    State(state): State<AppState>,
    Form(form): Form<ForgotPasswordForm>,
) -> Html<String> {
    if let (Some(auth), Some(pool)) = (state.auth.as_ref(), state.db.as_ref()) {
        let result = auth.create_password_reset_token(pool, &form.email).await;
        if let Ok(Some(token)) = result {
            if state.dev_mode {
                // In dev mode, show the reset link directly
                let link = format!("/auth/reset-password?token={}", token);
                return Html(format!(
                    r#"<div class="auth-page"><div class="auth-card" style="text-align: center;">
                    <h1 class="auth-title">رابط الاستعادة</h1>
                    <p class="auth-subtitle">وضع التطوير: رابط استعادة كلمة المرور</p>
                    <a href="{}" class="btn btn-primary">استعادة كلمة المرور</a>
                    </div></div>"#,
                    link
                ).into());
            }
            // Send password reset email via SMTP
            if let Some(mail) = state.mail.as_ref() {
                // Get user name for personalization
                let user_name: Option<(String,)> = sqlx::query_as("SELECT name FROM users WHERE email = $1")
                    .bind(&form.email)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();
                let name = user_name.map(|(n,)| n).unwrap_or_else(|| "User".to_string());
                if let Err(e) = mail.send_password_reset_email(&form.email, &name, &token).await {
                    warn!(error = %e, "Failed to send password reset email");
                }
            }
        }
    }
    Html(forgot_password_page(None, true))
}

async fn reset_password_page_handler(
    Query(params): Query<ResetPasswordQuery>,
) -> Html<String> {
    Html(reset_password_page(&params.token, None, false))
}

#[derive(Deserialize)]
struct ResetPasswordQuery {
    token: String,
}

#[derive(Deserialize)]
struct ResetPasswordForm {
    token: String,
    password: String,
}

async fn reset_password_form_handler(
    State(state): State<AppState>,
    Form(form): Form<ResetPasswordForm>,
) -> Html<String> {
    if let (Some(auth), Some(pool)) = (state.auth.as_ref(), state.db.as_ref()) {
        match auth.reset_password(pool, &form.token, &form.password).await {
            Ok(_) => return Html(reset_password_page("", None, true)),
            Err(e) => {
                let msg = match e {
                    AuthError::InvalidToken => "الرمز غير صالح",
                    AuthError::TokenExpired => "انتهت صلاحية الرمز",
                    AuthError::WeakPassword => "كلمة المرور ضعيفة (8 أحرف على الأقل)",
                    _ => "حدث خطأ",
                };
                return Html(reset_password_page(&form.token, Some(msg), false));
            }
        }
    }
    Html(reset_password_page(&form.token, Some("Service unavailable"), false))
}

async fn verify_email_handler(
    State(state): State<AppState>,
    Query(params): Query<VerifyEmailQuery>,
) -> Html<String> {
    let token = &params.token;
    if let (Some(auth), Some(pool)) = (state.auth.as_ref(), state.db.as_ref()) {
        match auth.verify_email(pool, token).await {
            Ok(_) => return Html(verify_email_page(true, None)),
            Err(e) => {
                let msg = match e {
                    AuthError::InvalidToken => "الرمز غير صالح أو مستخدم",
                    AuthError::TokenExpired => "انتهت صلاحية الرمز",
                    _ => "حدث خطأ",
                };
                return Html(verify_email_page(false, Some(msg)));
            }
        }
    }
    Html(verify_email_page(false, Some("Service unavailable")))
}

#[derive(Deserialize)]
struct VerifyEmailQuery {
    token: String,
}

#[derive(Deserialize)]
struct ChangePasswordForm {
    current_password: String,
    new_password: String,
}

async fn change_password_form_handler(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    if let (Some(auth), Some(pool)) = (state.auth.as_ref(), state.db.as_ref()) {
        match auth.change_password(pool, ctx.user.id, &form.current_password, &form.new_password).await {
            Ok(_) => return Redirect::to("/admin").into_response(),
            Err(e) => {
                let msg = match e {
                    AuthError::InvalidCredentials => "كلمة المرور الحالية غير صحيحة",
                    AuthError::WeakPassword => "كلمة المرور الجديدة ضعيفة",
                    _ => "حدث خطأ",
                };
                return (StatusCode::BAD_REQUEST, Html(format!("<h1>خطأ</h1><p>{}</p>", msg))).into_response();
            }
        }
    }
    (StatusCode::SERVICE_UNAVAILABLE, "Service unavailable").into_response()
}

// ─── OAuth Handlers ───

async fn oauth_google_handler(
    State(state): State<AppState>,
) -> Result<Redirect, Response> {
    let oauth = state.oauth_config.as_ref().ok_or_else(||
        (StatusCode::SERVICE_UNAVAILABLE, "OAuth not configured").into_response()
    )?;
    let state_token = generate_state();
    let url = build_auth_url(OAuthProvider::Google, oauth, &state_token)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
    // TODO: store state_token in a session cookie for CSRF verification on callback
    Ok(Redirect::to(&url))
}

async fn oauth_github_handler(
    State(state): State<AppState>,
) -> Result<Redirect, Response> {
    let oauth = state.oauth_config.as_ref().ok_or_else(||
        (StatusCode::SERVICE_UNAVAILABLE, "OAuth not configured").into_response()
    )?;
    let state_token = generate_state();
    let url = build_auth_url(OAuthProvider::Github, oauth, &state_token)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
    Ok(Redirect::to(&url))
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: String,
    state: Option<String>,
    error: Option<String>,
}

async fn oauth_callback_handler(
    State(state): State<AppState>,
    Path(provider_str): Path<String>,
    Query(params): Query<OAuthCallbackQuery>,
) -> Response {
    let provider = match OAuthProvider::from_str(&provider_str) {
        Some(p) => p,
        None => return Html(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">خطأ</h1><p class="auth-subtitle">مزود OAuth غير معروف</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#).into_response(),
    };

    // Check for OAuth error
    if let Some(err) = &params.error {
        let msg = format!("OAuth error: {}", err);
        return Html(format!(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">فشل OAuth</h1><p class="auth-subtitle">{}</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#, msg)).into_response();
    }

    let oauth_config = match state.oauth_config.as_ref() {
        Some(c) => c,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "OAuth not configured").into_response(),
    };

    let provider_config = match provider {
        OAuthProvider::Google => oauth_config.google.as_ref(),
        OAuthProvider::Github => oauth_config.github.as_ref(),
    };

    let Some(provider_config) = provider_config else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Provider not configured").into_response();
    };

    let http_client = match state.http_client.as_ref() {
        Some(c) => c,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "HTTP client not available").into_response(),
    };

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let auth = match state.auth.as_ref() {
        Some(a) => a,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "Auth not configured").into_response(),
    };

    // Step 1: Exchange code for access token
    let redirect_uri = format!(
        "{}/auth/oauth/{}/callback",
        oauth_config.redirect_base.trim_end_matches('/'),
        provider.as_str()
    );

    let token_request = serde_json::json!({
        "client_id": provider_config.client_id,
        "client_secret": provider_config.client_secret,
        "code": params.code,
        "redirect_uri": redirect_uri,
        "grant_type": "authorization_code",
    });

    let token_url = provider.token_url();
    let token_resp = match http_client.post(token_url)
        .header("Accept", "application/json")
        .json(&token_request)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "OAuth token exchange failed");
            return Html(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">فشل</h1><p class="auth-subtitle">تعذر تبادل الـ token</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#).into_response();
        }
    };

    let token_body: serde_json::Value = match token_resp.json().await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "OAuth token parse failed");
            return Html(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">فشل</h1><p class="auth-subtitle">تعذر قراءة الـ token</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#).into_response();
        }
    };

    let access_token = match token_body.get("access_token").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            error!(token_body = %token_body, "No access_token in OAuth response");
            return Html(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">فشل</h1><p class="auth-subtitle">لا يوجد access token</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#).into_response();
        }
    };

    // Step 2: Fetch user info
    let user_info_url = provider.user_info_url();
    let mut user_req = http_client.get(user_info_url)
        .header("Authorization", format!("Bearer {}", access_token));

    if provider == OAuthProvider::Github {
        user_req = user_req.header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "NovaX");
    }

    let user_resp = match user_req.send().await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "OAuth user info fetch failed");
            return Html(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">فشل</h1><p class="auth-subtitle">تعذر جلب بيانات المستخدم</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#).into_response();
        }
    };

    let user_info: serde_json::Value = match user_resp.json().await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "OAuth user info parse failed");
            return Html(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">فشل</h1><p class="auth-subtitle">تعذر قراءة بيانات المستخدم</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#).into_response();
        }
    };

    // Step 3: Extract email, name, provider_user_id
    let (email, name, provider_user_id, avatar_url) = match provider {
        OAuthProvider::Google => {
            let email = user_info.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = user_info.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let provider_user_id = user_info.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let avatar = user_info.get("picture").and_then(|v| v.as_str()).map(|s| s.to_string());
            (email, name, provider_user_id, avatar)
        }
        OAuthProvider::Github => {
            let email = user_info.get("email").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = user_info.get("name").and_then(|v| v.as_str())
                .or_else(|| user_info.get("login").and_then(|v| v.as_str()))
                .unwrap_or("").to_string();
            let provider_user_id = user_info.get("id").and_then(|v| v.as_i64()).map(|i| i.to_string())
                .unwrap_or_default();
            let avatar = user_info.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string());
            (email, name, provider_user_id, avatar)
        }
    };

    if email.is_empty() || provider_user_id.is_empty() {
        error!("OAuth: missing email or provider_user_id");
        return Html(r#"<div class="auth-page"><div class="auth-card" style="text-align:center;"><h1 class="auth-title">فشل</h1><p class="auth-subtitle">بيانات المستخدم غير مكتملة</p><a href="/auth/login" class="btn btn-primary">العودة</a></div></div>"#).into_response();
    }

    // Step 4: Find or create user
    // Check if oauth_account exists
    let existing_link: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM oauth_accounts WHERE provider = $1 AND provider_user_id = $2"
    )
    .bind(provider.as_str())
    .bind(&provider_user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let user_id = if let Some((uid,)) = existing_link {
        // Existing OAuth link — log in
        uid
    } else {
        // Check if user with same email exists
        let existing_user: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(&email)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

        let user_id = if let Some((uid,)) = existing_user {
            // Link OAuth to existing user
            uid
        } else {
            // Create new user
            let user: AuthUser = sqlx::query_as(
                r#"INSERT INTO users (email, name, avatar_url, password_hash, email_verified_at)
                   VALUES ($1, $2, $3, '', NOW())
                   RETURNING id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at"#,
            )
            .bind(&email)
            .bind(&name)
            .bind(&avatar_url)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| {
                panic!("Failed to create OAuth user")
            });
            user.id
        };

        // Create OAuth link
        let _ = sqlx::query(
            "INSERT INTO oauth_accounts (user_id, provider, provider_user_id) VALUES ($1, $2, $3)"
        )
        .bind(user_id)
        .bind(provider.as_str())
        .bind(&provider_user_id)
        .execute(pool)
        .await;

        user_id
    };

    // Step 5: Generate JWT + create session
    let user: AuthUser = sqlx::query_as(
        "SELECT id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at FROM users WHERE id = $1 AND is_active = TRUE",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .ok()
    .unwrap_or_else(|| panic!("User not found after OAuth"));

    let (token, expires_at) = match auth.generate_token(&user) {
        Ok((t, e)) => (t, e),
        Err(e) => {
            error!(error = %e, "Failed to generate token after OAuth");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed").into_response();
        }
    };

    // Store session
    let refresh_token = auth.generate_refresh_token();
    let _ = sqlx::query(
        "INSERT INTO auth_sessions (id, user_id, refresh_token, expires_at) VALUES ($1, $2, $3, $4)"
    )
    .bind(Uuid::new_v4())
    .bind(user.id)
    .bind(&refresh_token)
    .bind(expires_at + chrono::Duration::seconds(auth.config().refresh_ttl_seconds))
    .execute(pool)
    .await;

    info!(user_id = %user.id, provider = provider.as_str(), "OAuth login successful");

    // Set cookie + redirect to admin
    let cookie = format!(
        "novax_token={}; Path=/; HttpOnly; Max-Age=3600; SameSite=Lax",
        token
    );
    let mut response = Redirect::to("/admin").into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        cookie.parse().unwrap(),
    );
    response
}

// ─── API Auth Endpoints ───

async fn api_auth_me(axum::Extension(ctx): axum::Extension<AuthContext>) -> Json<AuthUser> {
    Json(ctx.user)
}

async fn api_auth_logout(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
) -> StatusCode {
    if let (Some(auth), Some(pool)) = (state.auth.as_ref(), state.db.as_ref()) {
        let _ = auth.logout(pool, ctx.user.id).await;
    }
    StatusCode::NO_CONTENT
}

// ─── Admin Dashboard Handlers ───

async fn admin_dashboard_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(pool).await.unwrap_or((0,));
    let verified: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_verified_at IS NOT NULL").fetch_one(pool).await.unwrap_or((0,));
    let active: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_active = TRUE").fetch_one(pool).await.unwrap_or((0,));
    let admins: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = TRUE").fetch_one(pool).await.unwrap_or((0,));

    // Project stats
    let projects_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM novax_projects").fetch_one(pool).await.unwrap_or((0,));

    // Recent users (5)
    let recent: Vec<UserRow> = sqlx::query_as("SELECT id, email, name, is_active, is_admin, email_verified_at, created_at FROM users ORDER BY created_at DESC LIMIT 5")
        .fetch_all(pool).await.unwrap_or_default();

    let recent_rows: String = recent.iter().map(|u| {
        let status_badge = if u.is_active { r#"<span class="badge badge-green">نشط</span>"# } else { r#"<span class="badge badge-red">موقوف</span>"# };
        format!(
            r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
            u.name, u.email, status_badge, u.created_at.format("%Y-%m-%d %H:%M")
        )
    }).collect();

    let stats = DashboardStats {
        total_users: total.0,
        verified_users: verified.0,
        active_users: active.0,
        admin_users: admins.0,
        recent_users_rows: recent_rows.clone(),
    };

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    let uptime = state.start_time.elapsed().as_secs();

    Html(format!(
        r##"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin" class="active"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/projects"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/profile"><span class="icon">👤</span> ملفي</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">مرحباً، {} 👋</h1>
    <p class="page-subtitle">إليك نظرة عامة على منصة Novax — وقت التشغيل: {}</p>

    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-label">👥 إجمالي المستخدمين</div>
        <div class="stat-value">{}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">✅ بريد مُحقَّق</div>
        <div class="stat-value">{}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">🟢 مستخدمون نشطون</div>
        <div class="stat-value">{}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">🔑 مسؤولون</div>
        <div class="stat-value">{}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">📦 مشاريع</div>
        <div class="stat-value">{}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">⏱️ وقت التشغيل</div>
        <div class="stat-value">{}m</div>
      </div>
    </div>

    <div style="display: grid; grid-template-columns: 2fr 1fr; gap: 24px;">
      <div class="card">
        <div class="card-header">
          <h3>أحدث المستخدمين</h3>
          <a href="/admin/users">عرض الكل ←</a>
        </div>
        <table>
          <thead><tr><th>الاسم</th><th>البريد</th><th>الحالة</th><th>التاريخ</th></tr></thead>
          <tbody>{recent}</tbody>
        </table>
      </div>
      <div class="card">
        <div class="card-header"><h3>إجراءات سريعة</h3></div>
        <div class="card-body" style="display: flex; flex-direction: column; gap: 12px;">
          <a href="/admin/projects" class="btn btn-primary" style="width: auto;">📦 إنشاء مشروع جديد</a>
          <a href="/admin/users" class="btn btn-secondary" style="width: auto;">👥 إدارة المستخدمين</a>
          <a href="/profile" class="btn btn-secondary" style="width: auto;">👤 ملفي الشخصي</a>
          <a href="/admin/settings" class="btn btn-secondary" style="width: auto;">⚙️ إعدادات المنصة</a>
        </div>
      </div>
    </div>
  </div>
</div>"##,
        ctx.user.name,
        format_uptime(uptime),
        total.0, verified.0, active.0, admins.0,
        projects_count.0,
        uptime / 60,
        admin_header = admin_header("لوحة التحكم", &ctx.user.email, initial),
        recent = if recent_rows.is_empty() {
            r#"<tr><td colspan="4" style="text-align:center;opacity:0.5;padding:24px;">لا يوجد مستخدمون بعد</td></tr>"#.to_string()
        } else {
            recent_rows
        },
    )).into_response()
}

fn format_uptime(secs: u64) -> String {
    if secs < 60 { format!("{}s", secs) }
    else if secs < 3600 { format!("{}m {}s", secs / 60, secs % 60) }
    else { format!("{}h {}m", secs / 3600, (secs % 3600) / 60) }
}

async fn admin_users_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };
    let page = params.page.unwrap_or(1).max(1);
    let per_page = 20u32;
    let offset = ((page - 1) * per_page) as i64;

    let users: Vec<UserRow> = sqlx::query_as(
        "SELECT id, email, name, is_active, is_admin, email_verified_at, created_at FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    )
    .bind(per_page as i64)
    .bind(offset)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(pool).await.unwrap_or((0,));
    let total_pages = ((total.0 as u32) + per_page - 1) / per_page;

    let rows = users.iter().map(|u| {
        let status = if u.is_active {
            r#"<span class="badge badge-green">نشط</span>"#
        } else {
            r#"<span class="badge badge-red">موقوف</span>"#
        };
        let role = if u.is_admin {
            r#"<span class="badge badge-yellow">مسؤول</span>"#
        } else {
            r#"<span class="badge badge-blue">مستخدم</span>"#
        };
        let verified = if u.email_verified_at.is_some() {
            r#"<span class="badge badge-green">✓</span>"#
        } else {
            r#"<span class="badge badge-red">✗</span>"#
        };
        format!(
            r#"<tr>
                <td>{name}</td><td>{email}</td><td>{status}</td><td>{role}</td><td>{verified}</td>
                <td>{created}</td>
                <td class="actions">
                    <form method="POST" action="/admin/users/{id}/toggle-active" style="display:inline;">
                        <button class="btn btn-secondary" style="width:auto; padding:6px 12px; font-size:12px;">{active_label}</button>
                    </form>
                    <form method="POST" action="/admin/users/{id}/toggle-admin" style="display:inline;">
                        <button class="btn btn-secondary" style="width:auto; padding:6px 12px; font-size:12px;">{admin_label}</button>
                    </form>
                    <form method="POST" action="/admin/delete-user/{id}" style="display:inline;" onsubmit="return confirm('هل أنت متأكد؟')">
                        <button class="btn btn-danger" style="width:auto; padding:6px 12px; font-size:12px;">حذف</button>
                    </form>
                </td>
            </tr>"#,
            name = u.name,
            email = u.email,
            status = status,
            role = role,
            verified = verified,
            created = u.created_at.format("%Y-%m-%d %H:%M"),
            id = u.id,
            active_label = if u.is_active { "إيقاف" } else { "تفعيل" },
            admin_label = if u.is_admin { "إزالة صلاحية" } else { "جعل مسؤول" },
        )
    }).collect::<String>();

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(admin_users_page(&ctx.user.email, initial, &rows, page, total_pages.max(1))).into_response()
}

async fn admin_user_detail_handler(
    axum::Extension(_ctx): axum::Extension<AuthContext>,
    Path(_id): Path<Uuid>,
) -> Response {
    // TODO: detailed user view
    (StatusCode::NOT_IMPLEMENTED, "User detail page coming soon").into_response()
}

async fn admin_toggle_active_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    if let Some(pool) = state.db.as_ref() {
        let _ = sqlx::query("UPDATE users SET is_active = NOT is_active, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool).await;
    }
    Redirect::to("/admin/users").into_response()
}

async fn admin_toggle_admin_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    // Prevent admin from removing their own admin status
    if ctx.user.id == id {
        return (StatusCode::BAD_REQUEST, "Cannot modify your own admin status").into_response();
    }
    if let Some(pool) = state.db.as_ref() {
        let _ = sqlx::query("UPDATE users SET is_admin = NOT is_admin, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool).await;
    }
    Redirect::to("/admin/users").into_response()
}

async fn admin_delete_user_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    // Prevent self-deletion
    if ctx.user.id == id {
        return (StatusCode::BAD_REQUEST, "Cannot delete your own account").into_response();
    }
    if let Some(pool) = state.db.as_ref() {
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(pool).await;
    }
    Redirect::to("/admin/users").into_response()
}

async fn admin_settings_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    let rl = state.rate_limiter.as_ref();
    let rl_enabled = rl.is_some();
    let (rl_max, rl_window) = if let Some(limiter) = rl {
        (limiter.config().max_requests, limiter.config().window_seconds)
    } else {
        (100, 60)
    };
    let google = state.oauth_config.as_ref().is_some_and(|c| c.google_enabled());
    let github = state.oauth_config.as_ref().is_some_and(|c| c.github_enabled());

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(admin_settings_page(&ctx.user.email, initial, rl_enabled, rl_max, rl_window, google, github, None)).into_response()
}

#[derive(Deserialize)]
struct SettingsForm {
    rate_limit_enabled: Option<String>,
    rate_limit_max: Option<u32>,
    rate_limit_window: Option<u64>,
    google_enabled: Option<String>,
    github_enabled: Option<String>,
}

async fn admin_settings_form_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Form(form): Form<SettingsForm>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    // For v0.4: settings are mostly informational (env-based for now).
    // Future versions will persist to DB and apply at runtime.
    let rl_enabled = form.rate_limit_enabled.as_deref() == Some("true");
    let google = form.google_enabled.as_deref() == Some("true");
    let github = form.github_enabled.as_deref() == Some("true");
    let rl_max = form.rate_limit_max.unwrap_or(100);
    let rl_window = form.rate_limit_window.unwrap_or(60);

    info!(rl_enabled, google, github, rl_max, rl_window, "Settings updated (informational — env-based for v0.4)");

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(admin_settings_page(&ctx.user.email, initial, rl_enabled, rl_max, rl_window, google, github, Some("تم حفظ الإعدادات (ستُطبَّق عند إعادة التشغيل)"))).into_response()
}

// ─── Avatar Upload Handler ───

async fn upload_avatar_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("avatar") {
            let file_name = field.file_name().unwrap_or("avatar.png").to_string();
            let data = match field.bytes().await {
                Ok(d) => d,
                Err(e) => return (StatusCode::BAD_REQUEST, format!("read error: {}", e)).into_response(),
            };

            // Validate: max 2MB, image types only
            if data.len() > 2 * 1024 * 1024 {
                return (StatusCode::PAYLOAD_TOO_LARGE, "File too large (max 2MB)").into_response();
            }

            let ext = file_name.rsplit('.').next().unwrap_or("png").to_lowercase();
            let ext = match ext.as_str() {
                "png" | "jpg" | "jpeg" | "gif" | "webp" => ext,
                _ => "png".to_string(),
            };

            // Save to /uploads/{user_id}.{ext}
            let file_path = format!("uploads/{}.{}", ctx.user.id, ext);
            if let Err(e) = tokio::fs::write(&file_path, &data).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("save error: {}", e)).into_response();
            }

            // Update user avatar_url
            if let Some(pool) = state.db.as_ref() {
                let url = format!("/uploads/{}.{}", ctx.user.id, ext);
                let _ = sqlx::query("UPDATE users SET avatar_url = $1, updated_at = NOW() WHERE id = $2")
                    .bind(&url)
                    .bind(ctx.user.id)
                    .execute(pool).await;
            }
            return (StatusCode::OK, "Avatar uploaded").into_response();
        }
    }
    (StatusCode::BAD_REQUEST, "No avatar field").into_response()
}

// ─── Profile + User Edit Handlers ───

/// GET /profile — user's own profile page
async fn profile_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
) -> Response {
    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(profile_page(&ctx.user.email, initial, &ctx.user, None)).into_response()
}

#[derive(Deserialize)]
struct ProfileUpdateForm {
    name: Option<String>,
    bio: Option<String>,
}

/// POST /profile/update — update own profile
async fn profile_update_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Form(form): Form<ProfileUpdateForm>,
) -> Response {
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let updated: AuthUser = sqlx::query_as(
        r#"UPDATE users SET name = COALESCE($2, name), bio = COALESCE($3, bio), updated_at = NOW()
           WHERE id = $1
           RETURNING id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at"#,
    )
    .bind(ctx.user.id)
    .bind(form.name.as_deref().filter(|s| !s.is_empty()))
    .bind(form.bio.as_deref().filter(|s| !s.is_empty()))
    .fetch_one(pool)
    .await
    .unwrap_or(ctx.user.clone());

    let initial = updated.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(profile_page(&updated.email, initial, &updated, Some("تم تحديث ملفك الشخصي بنجاح"))).into_response()
}

/// GET /admin/users/:id/edit — admin edit user page
async fn admin_user_edit_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let user: Option<AuthUser> = sqlx::query_as(
        "SELECT id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at FROM users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let Some(user) = user else {
        return (StatusCode::NOT_FOUND, "User not found").into_response();
    };

    let admin_initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(admin_user_edit_page(&ctx.user.email, admin_initial, &user, None)).into_response()
}

#[derive(Deserialize)]
struct AdminUpdateUserForm {
    name: String,
    email: String,
    bio: Option<String>,
    avatar_url: Option<String>,
    is_active: Option<String>,
    is_admin: Option<String>,
}

/// POST /admin/users/:id/update — admin updates user
async fn admin_user_update_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Form(form): Form<AdminUpdateUserForm>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    // Self-protection: cannot remove own admin
    if ctx.user.id == id && form.is_admin.as_deref() != Some("true") {
        return (StatusCode::BAD_REQUEST, "Cannot remove your own admin status").into_response();
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let is_active = form.is_active.as_deref() == Some("true");
    let is_admin = form.is_admin.as_deref() == Some("true");

    let updated: AuthUser = sqlx::query_as(
        r#"UPDATE users SET
            name = $2, email = $3, bio = $4, avatar_url = $5,
            is_active = $6, is_admin = $7, updated_at = NOW()
           WHERE id = $1
           RETURNING id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at"#,
    )
    .bind(id)
    .bind(&form.name)
    .bind(&form.email)
    .bind(form.bio.as_deref().filter(|s| !s.is_empty()))
    .bind(form.avatar_url.as_deref().filter(|s| !s.is_empty()))
    .bind(is_active)
    .bind(is_admin)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| ctx.user.clone());

    let admin_initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(admin_user_edit_page(&ctx.user.email, admin_initial, &updated, Some("تم تحديث المستخدم بنجاح"))).into_response()
}

// ─── Novax Engine: Project Management Handlers ───

/// GET /admin/projects — قائمة المشاريع
async fn admin_projects_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let projects: Vec<ProjectRow> = sqlx::query_as(
        "SELECT id, name, display_name, description, enabled, created_at, updated_at FROM novax_projects ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let rows = projects.iter().map(|p| {
        format!(
            r#"<tr>
                <td><a href="/admin/projects/{}"><strong>{}</strong></a></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td class="actions">
                    <a href="/admin/projects/{}/preview" class="btn btn-secondary btn-sm">معاينة</a>
                    <a href="/admin/projects/{}/export" class="btn btn-primary btn-sm">تصدير</a>
                </td>
            </tr>"#,
            p.id, p.name, p.display_name,
            p.description.as_deref().unwrap_or("—"),
            p.created_at.format("%Y-%m-%d"),
            p.id, p.id,
        )
    }).collect::<String>();

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/projects" class="active"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">المشاريع</h1>
    <p class="page-subtitle">إدارة مشاريع Novax — منشئ التطبيقات الموجّه بالنيّة</p>

    <div class="card" style="margin-bottom: 24px;">
      <div class="card-header"><h3>إنشاء مشروع جديد</h3></div>
      <div class="card-body">
        <form method="POST" action="/admin/projects">
          <div class="form-row">
            <div class="form-group">
              <label>اسم المشروع (PascalCase)</label>
              <input type="text" name="name" required placeholder="MyStore" pattern="[A-Z][a-zA-Z0-9]*">
            </div>
            <div class="form-group">
              <label>الاسم المعروض (عربي)</label>
              <input type="text" name="display_name" required placeholder="متجري">
            </div>
          </div>
          <div class="form-group">
            <label>الوصف (اختياري)</label>
            <textarea name="description" rows="2" placeholder="وصف موجز للمشروع..."></textarea>
          </div>
          <button type="submit" class="btn btn-primary" style="width: auto;">+ إنشاء المشروع</button>
        </form>
      </div>
    </div>

    <div class="card">
      <div class="card-header"><h3>المشاريع الحالية</h3></div>
      <table class="table">
        <thead>
          <tr><th>الاسم</th><th>الاسم المعروض</th><th>الوصف</th><th>تاريخ الإنشاء</th><th>إجراءات</th></tr>
        </thead>
        <tbody>
          {rows}
        </tbody>
      </table>
    </div>
  </div>
</div>"#,
        admin_header = admin_header("المشاريع", &ctx.user.email, initial),
        rows = if rows.is_empty() {
            r#"<tr><td colspan="5" style="text-align: center; opacity: 0.5; padding: 32px;">لا توجد مشاريع بعد. أنشئ أول مشروع أعلاه.</td></tr>"#.to_string()
        } else {
            rows
        },
    )).into_response()
}

#[derive(Debug, Deserialize)]
struct CreateProjectForm {
    name: String,
    display_name: String,
    description: Option<String>,
}

/// POST /admin/projects — إنشاء مشروع جديد
async fn admin_create_project_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Form(form): Form<CreateProjectForm>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let project = ProjectConfig::new(&form.name, &form.display_name);
    let mut project = project;
    project.description = form.description;

    let config_json = match project.to_json() {
        Ok(j) => j,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON error: {}", e)).into_response(),
    };

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let result = sqlx::query(
        "INSERT INTO novax_projects (id, name, display_name, description, config) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(project.id)
    .bind(&project.name)
    .bind(&project.display_name)
    .bind(&project.description)
    .bind(&config_json)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Redirect::to(&format!("/admin/projects/{}", project.id)).into_response(),
        Err(e) => {
            let msg = if e.to_string().contains("unique") {
                "اسم المشروع موجود بالفعل"
            } else {
                "حدث خطأ أثناء الإنشاء"
            };
            (StatusCode::BAD_REQUEST, msg).into_response()
        }
    }
}

/// GET /admin/projects/:id — تفاصيل المشروع + الكيانات
async fn admin_project_detail_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let row: Option<(String,)> = sqlx::query_as("SELECT config FROM novax_projects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    let Some((config_json,)) = row else {
        return (StatusCode::NOT_FOUND, "Project not found").into_response();
    };

    let project: ProjectConfig = match ProjectConfig::from_json(&config_json) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Parse error: {}", e)).into_response(),
    };

    let entities_html = project.entities.iter().map(|e| {
        format!(
            r#"<div class="card" style="margin-bottom: 12px;">
                <div style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h3 style="display: inline;">{} {}</h3>
                        <span style="color: var(--text-muted); margin-right: 8px;">({} حقل)</span>
                    </div>
                    <div class="actions">
                        <a href="/admin/projects/{}/preview" class="btn btn-secondary btn-sm">معاينة</a>
                    </div>
                </div>
                <div style="margin-top: 8px; color: var(--text-muted); font-size: 13px;">
                    الجدول: <code>{}</code> · المسار: <code>/{}'</code>
                </div>
            </div>"#,
            e.icon, e.display_name, e.fields.len(),
            id, e.table(), e.route_prefix(),
        )
    }).collect::<String>();

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/projects" class="active"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">{name}</h1>
    <p class="page-subtitle">{desc}</p>

    <div style="display: flex; gap: 12px; margin-bottom: 24px;">
      <a href="/admin/projects/{id}/preview" class="btn btn-primary">🔍 معاينة مباشرة</a>
      <a href="/admin/projects/{id}/export" class="btn btn-secondary">📥 تصدير للإنتاج</a>
    </div>

    <div class="card" style="margin-bottom: 24px;">
      <div class="card-header">
        <h3>إضافة كيان جديد</h3>
      </div>
      <div class="card-body">
        <form method="POST" action="/admin/projects/{id}/entities">
          <div class="form-row">
            <div class="form-group">
              <label>اسم الكيان (PascalCase)</label>
              <input type="text" name="name" required placeholder="Product" pattern="[A-Z][a-zA-Z0-9]*">
            </div>
            <div class="form-group">
              <label>الاسم (مفرد)</label>
              <input type="text" name="display_name" required placeholder="منتج">
            </div>
            <div class="form-group">
              <label>الاسم (جمع)</label>
              <input type="text" name="display_name_plural" required placeholder="منتجات">
            </div>
            <div class="form-group">
              <label>الأيقونة (emoji)</label>
              <input type="text" name="icon" value="📦" maxlength="4">
            </div>
          </div>
          <button type="submit" class="btn btn-primary" style="width: auto;">+ إضافة كيان</button>
        </form>
      </div>
    </div>

    <h3 style="margin-bottom: 12px;">الكيانات ({count})</h3>
    {entities}
  </div>
</div>"#,
        admin_header = admin_header("تفاصيل المشروع", &ctx.user.email, initial),
        name = project.display_name,
        desc = project.description.as_deref().unwrap_or("لا يوجد وصف"),
        id = id,
        count = project.entities.len(),
        entities = if entities_html.is_empty() {
            r#"<div class="empty-state"><p>لا توجد كيانات بعد. أضف أول كيان أعلاه.</p></div>"#.to_string()
        } else {
            entities_html
        },
    )).into_response()
}

#[derive(Debug, Deserialize)]
struct AddEntityForm {
    name: String,
    display_name: String,
    display_name_plural: String,
    icon: String,
}

/// POST /admin/projects/:id/entities — إضافة كيان
async fn admin_add_entity_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Form(form): Form<AddEntityForm>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let row: Option<(String,)> = sqlx::query_as("SELECT config FROM novax_projects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    let Some((config_json,)) = row else {
        return (StatusCode::NOT_FOUND, "Project not found").into_response();
    };

    let mut project: ProjectConfig = match ProjectConfig::from_json(&config_json) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Parse error: {}", e)).into_response(),
    };

    let entity = EntityConfig::new(&form.name, &form.display_name, &form.display_name_plural);
    let mut entity = entity;
    entity.icon = form.icon;
    project.add_entity(entity);

    let config_json = match project.to_json() {
        Ok(j) => j,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON error: {}", e)).into_response(),
    };

    let _ = sqlx::query("UPDATE novax_projects SET config = $1, updated_at = NOW() WHERE id = $2")
        .bind(&config_json)
        .bind(id)
        .execute(pool)
        .await;

    Redirect::to(&format!("/admin/projects/{}", id)).into_response()
}

/// GET /admin/projects/:id/export — تصدير المشروع (عرض الكود المُولَّد)
async fn admin_export_project_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let row: Option<(String,)> = sqlx::query_as("SELECT config FROM novax_projects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    let Some((config_json,)) = row else {
        return (StatusCode::NOT_FOUND, "Project not found").into_response();
    };

    let project: ProjectConfig = match ProjectConfig::from_json(&config_json) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Parse error: {}", e)).into_response(),
    };

    let files = match build_project(&project) {
        Ok(f) => f,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Compile error: {}", e)).into_response(),
    };

    // عرض الملفات المُولَّدة في صفحة HTML
    let files_html = files.iter().map(|f| {
        format!(
            r#"<div class="card" style="margin-bottom: 16px;">
                <div class="card-header">
                    <h3><code>{}</code></h3>
                    <button class="btn btn-secondary btn-sm" onclick="copyToClipboard('content-{}')">نسخ</button>
                </div>
                <div class="card-body">
                    <pre id="content-{}" style="background: #0a0a0b; padding: 16px; border-radius: 8px; overflow-x: auto; font-size: 13px; line-height: 1.5; direction: ltr; text-align: left;"><code>{}</code></pre>
                </div>
            </div>"#,
            f.path,
            f.path.replace('/', "-").replace('.', "-"),
            f.path.replace('/', "-").replace('.', "-"),
            escape_html(&f.content),
        )
    }).collect::<String>();

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/projects" class="active"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">تصدير: {name}</h1>
    <p class="page-subtitle">{count} ملف — كود Rust + HTML + SQL + CSS مستقل تمامًا عن Novax</p>
    <div class="alert alert-success" style="margin-bottom: 24px;">
      ✅ الكود المُولَّد مستقل عن Novax. يمكن نسخه وتشغيله بأمر <code>cargo run --release</code> على أي خادم.
    </div>
    {files}
  </div>
</div>
<script>
function copyToClipboard(id) {{
  const text = document.getElementById(id).innerText;
  navigator.clipboard.writeText(text);
}}
</script>"#,
        admin_header = admin_header("تصدير المشروع", &ctx.user.email, initial),
        name = project.display_name,
        count = files.len(),
        files = files_html,
    )).into_response()
}

/// GET /admin/projects/:id/preview — معاينة الكود المُولَّد
async fn admin_preview_project_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let row: Option<(String,)> = sqlx::query_as("SELECT config FROM novax_projects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    let Some((config_json,)) = row else {
        return (StatusCode::NOT_FOUND, "Project not found").into_response();
    };

    let project: ProjectConfig = match ProjectConfig::from_json(&config_json) {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Parse error: {}", e)).into_response(),
    };

    let css = project.theme.to_css();
    let entity_links: String = project.entities.iter()
        .map(|e| format!(r##"<a href="#" class="btn btn-secondary">{icon} {name}</a>"##, icon = e.icon, name = e.display_name_plural))
        .collect::<Vec<_>>()
        .join("\n    ");

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(format!(
        r#"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/projects" class="active"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">معاينة: {name}</h1>
    <p class="page-subtitle">معاينة مباشرة للكود المُولَّد (CSS + HTML من الإعدادات)</p>

    <div class="card" style="margin-bottom: 24px;">
      <div class="card-header"><h3>🎨 المعاينة (CSS من إعدادات المظهر)</h3></div>
      <div class="card-body">
        <style>{css}</style>
        <div style="background: var(--color-bg); color: var(--color-text); padding: 20px; border-radius: var(--radius); direction: var(--dir);">
          <nav class="nav">
            {entity_links}
          </nav>
          <div class="page-header">
            <h1>عنوان الصفحة</h1>
            <button class="btn btn-primary">+ إضافة جديدة</button>
          </div>
          <table class="table">
            <thead>
              <tr><th>العمود الأول</th><th>العمود الثاني</th><th>الحالة</th><th>إجراءات</th></tr>
            </thead>
            <tbody>
              <tr>
                <td>عنصر 1</td>
                <td>قيمة</td>
                <td><span class="badge badge-success">✓ نشط</span></td>
                <td class="actions"><button class="btn btn-secondary btn-sm">عرض</button><button class="btn btn-danger btn-sm">حذف</button></td>
              </tr>
              <tr>
                <td>عنصر 2</td>
                <td>قيمة</td>
                <td><span class="badge badge-danger">✗ معطّل</span></td>
                <td class="actions"><button class="btn btn-secondary btn-sm">عرض</button><button class="btn btn-danger btn-sm">حذف</button></td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-header"><h3>📥 جاهز للتصدير</h3></div>
      <div class="card-body">
        <p>المشروع يحتوي على <strong>{entity_count}</strong> كيان و <strong>{field_count}</strong> حقل إجمالاً.</p>
        <a href="/admin/projects/{id}/export" class="btn btn-primary">📥 تصدير الكود الكامل</a>
      </div>
    </div>
  </div>
</div>"#,
        admin_header = admin_header("معاينة المشروع", &ctx.user.email, initial),
        name = project.display_name,
        css = css,
        entity_links = if entity_links.is_empty() { "لا توجد كيانات".to_string() } else { entity_links },
        entity_count = project.entities.len(),
        field_count = project.entities.iter().map(|e| e.fields.len()).sum::<usize>(),
        id = id,
    )).into_response()
}

#[derive(Debug, sqlx::FromRow)]
struct ProjectRow {
    id: Uuid,
    name: String,
    display_name: String,
    description: Option<String>,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

// ─── Entity Field Editor Handlers ───

/// GET /admin/projects/:id/entities/:entity_id/fields — محرّر الحقول
async fn admin_entity_fields_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path((project_id, entity_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let project = match load_project(pool, project_id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    let entity = match project.find_entity_by_id(entity_id) {
        Some(e) => e,
        None => return (StatusCode::NOT_FOUND, "Entity not found").into_response(),
    };

    let fields_html = entity.fields.iter().map(|f| {
        let type_str = match &f.field_type {
            FieldType::Uuid => "UUID",
            FieldType::String => "String",
            FieldType::Text => "Text",
            FieldType::Integer => "Integer",
            FieldType::Decimal => "Decimal",
            FieldType::Boolean => "Boolean",
            FieldType::Timestamp => "Timestamp",
            FieldType::Date => "Date",
            FieldType::Json => "JSON",
            FieldType::Reference => "Reference",
        };
        let badges = format!(
            r#"{}{}{}{}"#,
            if f.primary_key { r#"<span class="badge badge-yellow">PK</span> "# } else { "" },
            if f.required { r#"<span class="badge badge-blue">مطلوب</span> "# } else { "" },
            if f.auto_generate { r#"<span class="badge badge-green">تلقائي</span> "# } else { "" },
            if f.display_in_list { r#"<span class="badge badge-success">قائمة</span>"# } else { "" },
        );
        format!(
            r##"<tr>
                <td><code>{}</code></td>
                <td><span class="badge badge-blue">{}</span></td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td class="actions">
                    <form method="POST" action="/admin/projects/{}/entities/{}/fields/{}/delete"
                          style="display:inline;"
                          onsubmit="return confirm('حذف الحقل «{}»؟')">
                        <button class="btn btn-danger btn-sm">حذف</button>
                    </form>
                </td>
            </tr>"##,
            f.name, type_str, f.label, badges,
            f.description.as_deref().unwrap_or("—"),
            project_id, entity_id, f.name, f.name,
        )
    }).collect::<String>();

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(format!(
        r##"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/projects" class="active"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">{} {} — الحقول</h1>
    <p class="page-subtitle">{} حقل · الجدول: <code>{}</code></p>
    <div style="display: flex; gap: 12px; margin-bottom: 24px;">
      <a href="/admin/projects/{}" class="btn btn-secondary">← العودة للمشروع</a>
      <a href="/admin/projects/{}/preview" class="btn btn-primary">معاينة</a>
      <a href="/admin/projects/{}/export" class="btn btn-secondary">تصدير</a>
      <a href="/admin/projects/{}/download" class="btn btn-primary">📥 تحميل tar.gz</a>
    </div>

    <div class="card" style="margin-bottom: 24px;">
      <div class="card-header"><h3>➕ إضافة حقل جديد</h3></div>
      <div class="card-body">
        <form method="POST" action="/admin/projects/{}/entities/{}/fields/add">
          <div class="form-row">
            <div class="form-group">
              <label>اسم الحقل (snake_case)</label>
              <input type="text" name="name" required placeholder="title" pattern="[a-z][a-z0-9_]*">
            </div>
            <div class="form-group">
              <label>النوع</label>
              <select name="field_type">
                <option value="string">String (نص قصير)</option>
                <option value="text">Text (نص طويل)</option>
                <option value="integer">Integer (عدد صحيح)</option>
                <option value="decimal">Decimal (رقم عشري)</option>
                <option value="boolean">Boolean (نعم/لا)</option>
                <option value="timestamp">Timestamp (تاريخ ووقت)</option>
                <option value="date">Date (تاريخ)</option>
                <option value="json">JSON</option>
              </select>
            </div>
            <div class="form-group">
              <label>التسمية (عربي)</label>
              <input type="text" name="label" required placeholder="العنوان">
            </div>
          </div>
          <div class="form-row">
            <div class="form-group">
              <label style="display:flex;align-items:center;gap:8px;">
                <input type="checkbox" name="required" value="true" checked> مطلوب (NOT NULL)
              </label>
            </div>
            <div class="form-group">
              <label style="display:flex;align-items:center;gap:8px;">
                <input type="checkbox" name="display_in_list" value="true" checked> يظهر في القائمة
              </label>
            </div>
            <div class="form-group">
              <label style="display:flex;align-items:center;gap:8px;">
                <input type="checkbox" name="display_in_form" value="true" checked> يظهر في النموذج
              </label>
            </div>
            <div class="form-group">
              <label style="display:flex;align-items:center;gap:8px;">
                <input type="checkbox" name="searchable" value="true"> قابل للبحث
              </label>
            </div>
          </div>
          <div class="form-group">
            <label>وصف اختياري</label>
            <input type="text" name="description" placeholder="وصف موجز للحقل">
          </div>
          <button type="submit" class="btn btn-primary" style="width: auto;">+ إضافة الحقل</button>
        </form>
      </div>
    </div>

    <div class="card">
      <div class="card-header"><h3>الحقول الحالية</h3></div>
      <table class="table">
        <thead>
          <tr><th>الاسم</th><th>النوع</th><th>التسمية</th><th>الخصائص</th><th>الوصف</th><th>إجراءات</th></tr>
        </thead>
        <tbody>
          {fields}
        </tbody>
      </table>
    </div>
  </div>
</div>"##,
        entity.icon, entity.display_name,
        entity.fields.len(), entity.table(),
        project_id, project_id, project_id, project_id,
        project_id, entity_id,
        admin_header = admin_header("محرّر الحقول", &ctx.user.email, initial),
        fields = if fields_html.is_empty() {
            r#"<tr><td colspan="6" style="text-align:center;opacity:0.5;padding:32px;">لا توجد حقول مخصصة.</td></tr>"#.to_string()
        } else {
            fields_html
        },
    )).into_response()
}

#[derive(Debug, Deserialize)]
struct AddFieldForm {
    name: String,
    field_type: String,
    label: String,
    required: Option<String>,
    display_in_list: Option<String>,
    display_in_form: Option<String>,
    searchable: Option<String>,
    description: Option<String>,
}

/// POST /admin/projects/:id/entities/:entity_id/fields/add — إضافة حقل
async fn admin_add_field_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path((project_id, entity_id)): Path<(Uuid, Uuid)>,
    Form(form): Form<AddFieldForm>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let mut project = match load_project(pool, project_id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    // Find the entity
    let entity_idx = project.entities.iter().position(|e| e.id == entity_id);
    let Some(idx) = entity_idx else {
        return (StatusCode::NOT_FOUND, "Entity not found").into_response();
    };

    // Parse field type
    let field_type = match form.field_type.parse::<FieldType>() {
        Ok(t) => t,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Invalid field type: {}", e)).into_response(),
    };

    let field = FieldConfig {
        name: form.name,
        field_type: field_type.clone(),
        label: form.label,
        primary_key: false,
        auto_generate: false,
        required: form.required.as_deref() == Some("true"),
        nullable: form.required.as_deref() != Some("true"),
        max_length: if field_type == FieldType::String { Some(255) } else { None },
        precision: if field_type == FieldType::Decimal { Some(10) } else { None },
        scale: if field_type == FieldType::Decimal { Some(2) } else { None },
        default_value: None,
        display_in_list: form.display_in_list.as_deref() == Some("true"),
        display_in_form: form.display_in_form.as_deref() == Some("true"),
        display_in_detail: true,
        searchable: form.searchable.as_deref() == Some("true"),
        sortable: true,
        references: None,
        description: form.description,
    };

    project.entities[idx].fields.push(field);

    if let Err(e) = save_project(pool, &project).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Save error: {}", e)).into_response();
    }

    Redirect::to(&format!("/admin/projects/{}/entities/{}/fields", project_id, entity_id)).into_response()
}

/// POST /admin/projects/:id/entities/:entity_id/fields/:field_name/delete — حذف حقل
async fn admin_delete_field_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path((project_id, entity_id, field_name)): Path<(Uuid, Uuid, String)>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let mut project = match load_project(pool, project_id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    let entity_idx = project.entities.iter().position(|e| e.id == entity_id);
    let Some(idx) = entity_idx else {
        return (StatusCode::NOT_FOUND, "Entity not found").into_response();
    };

    // Don't delete primary key
    if field_name == "id" || field_name == "created_at" || field_name == "updated_at" {
        return (StatusCode::BAD_REQUEST, "Cannot delete system field").into_response();
    }

    let before = project.entities[idx].fields.len();
    project.entities[idx].fields.retain(|f| f.name != field_name);
    if project.entities[idx].fields.len() == before {
        return (StatusCode::NOT_FOUND, "Field not found").into_response();
    }

    let _ = save_project(pool, &project).await;
    Redirect::to(&format!("/admin/projects/{}/entities/{}/fields", project_id, entity_id)).into_response()
}

/// POST /admin/projects/:id/entities/:entity_id/delete — حذف كيان
async fn admin_delete_entity_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path((project_id, entity_id)): Path<(Uuid, Uuid)>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let mut project = match load_project(pool, project_id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    project.remove_entity(entity_id);
    let _ = save_project(pool, &project).await;
    Redirect::to(&format!("/admin/projects/{}", project_id)).into_response()
}

// ─── Theme Editor Handler ───

/// GET /admin/projects/:id/theme — محرّر المظهر
async fn admin_theme_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let project = match load_project(pool, id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    let theme = &project.theme;
    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    let css = theme.to_css();

    Html(format!(
        r##"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/projects" class="active"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">🎨 محرّر المظهر</h1>
    <p class="page-subtitle">{name}</p>
    <div style="display: flex; gap: 12px; margin-bottom: 24px;">
      <a href="/admin/projects/{id}" class="btn btn-secondary">← العودة للمشروع</a>
      <a href="/admin/projects/{id}/preview" class="btn btn-primary">🔍 معاينة</a>
    </div>
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 24px;">
      <div class="card">
        <div class="card-header"><h3>الإعدادات</h3></div>
        <div class="card-body">
          <form method="POST" action="/admin/projects/{id}/theme/update">
            <div class="form-group">
              <label>اللون الأساسي</label>
              <input type="color" name="primary_color" value="{primary}" style="width:100%;height:48px;border:none;border-radius:8px;background:none;cursor:pointer;">
            </div>
            <div class="form-group">
              <label>لون الخلفية</label>
              <input type="color" name="bg_color" value="{bg}" style="width:100%;height:48px;border:none;border-radius:8px;background:none;cursor:pointer;">
            </div>
            <div class="form-group">
              <label>لون النص</label>
              <input type="color" name="text_color" value="{text}" style="width:100%;height:48px;border:none;border-radius:8px;background:none;cursor:pointer;">
            </div>
            <div class="form-group">
              <label>لون الأسطح (البطاقات)</label>
              <input type="color" name="surface_color" value="{surface}" style="width:100%;height:48px;border:none;border-radius:8px;background:none;cursor:pointer;">
            </div>
            <div class="form-group">
              <label>لون النجاح</label>
              <input type="color" name="success_color" value="{success}" style="width:100%;height:48px;border:none;border-radius:8px;background:none;cursor:pointer;">
            </div>
            <div class="form-group">
              <label>لون الخطر</label>
              <input type="color" name="danger_color" value="{danger}" style="width:100%;height:48px;border:none;border-radius:8px;background:none;cursor:pointer;">
            </div>
            <div class="form-group">
              <label>نصف قطر الزاوية</label>
              <input type="text" name="border_radius" value="{radius}" placeholder="8px">
            </div>
            <div class="form-group">
              <label>الخط</label>
              <input type="text" name="font_family" value="{font}" placeholder="system-ui, sans-serif">
            </div>
            <div class="form-group">
              <label>الاتجاه</label>
              <select name="direction">
                <option value="rtl" {rtl_selected}>RTL (عربي)</option>
                <option value="ltr" {ltr_selected}>LTR (إنجليزي)</option>
              </select>
            </div>
            <button type="submit" class="btn btn-primary" style="width: auto;">💾 حفظ المظهر</button>
          </form>
        </div>
      </div>
      <div class="card">
        <div class="card-header"><h3>معاينة حية</h3></div>
        <div class="card-body">
          <style>{css}</style>
          <div style="background: var(--color-bg); color: var(--color-text); padding: 16px; border-radius: var(--radius); direction: var(--dir);">
            <button class="btn btn-primary" style="width:auto;margin-bottom:8px;">زر أساسي</button>
            <button class="btn btn-secondary" style="width:auto;margin-bottom:8px;">زر ثانوي</button>
            <button class="btn btn-danger" style="width:auto;margin-bottom:8px;">زر خطر</button>
            <table class="table">
              <thead><tr><th>عمود</th><th>قيمة</th><th>حالة</th></tr></thead>
              <tbody>
                <tr><td>عنصر 1</td><td>100</td><td><span class="badge badge-success">✓</span></td></tr>
                <tr><td>عنصر 2</td><td>200</td><td><span class="badge badge-danger">✗</span></td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>"##,
        admin_header = admin_header("محرّر المظهر", &ctx.user.email, initial),
        name = project.display_name,
        id = id,
        primary = &theme.primary_color,
        bg = &theme.bg_color,
        text = &theme.text_color,
        surface = &theme.surface_color,
        success = &theme.success_color,
        danger = &theme.danger_color,
        radius = &theme.border_radius,
        font = &theme.font_family,
        rtl_selected = if theme.direction == "rtl" { "selected" } else { "" },
        ltr_selected = if theme.direction == "ltr" { "selected" } else { "" },
        css = css,
    )).into_response()
}

#[derive(Debug, Deserialize)]
struct ThemeUpdateForm {
    primary_color: String,
    bg_color: String,
    text_color: String,
    surface_color: String,
    success_color: String,
    danger_color: String,
    border_radius: String,
    font_family: String,
    direction: String,
}

/// POST /admin/projects/:id/theme/update — تحديث المظهر
async fn admin_theme_update_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Form(form): Form<ThemeUpdateForm>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let mut project = match load_project(pool, id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    project.theme.primary_color = form.primary_color;
    project.theme.bg_color = form.bg_color;
    project.theme.text_color = form.text_color;
    project.theme.surface_color = form.surface_color;
    project.theme.success_color = form.success_color;
    project.theme.danger_color = form.danger_color;
    project.theme.border_radius = form.border_radius;
    project.theme.font_family = form.font_family;
    project.theme.direction = form.direction.clone();
    project.theme.lang = if form.direction == "rtl" { "ar".to_string() } else { "en".to_string() };

    let _ = save_project(pool, &project).await;
    Redirect::to(&format!("/admin/projects/{}/theme", id)).into_response()
}

// ─── Download Project as tar.gz ───

/// GET /admin/projects/:id/download — تحميل المشروع كـ tar.gz
async fn admin_download_project_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let project = match load_project(pool, id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    let files = match build_project(&project) {
        Ok(f) => f,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Compile error: {}", e)).into_response(),
    };

    // إنشاء tar.gz في الذاكرة
    use std::io::Cursor;
    let mut tar_buffer = Cursor::new(Vec::new());
    {
        let mut tar_builder = tar::Builder::new(&mut tar_buffer);
        for file in &files {
            let mut header = tar::Header::new_gnu();
            header.set_path(format!("{}/{}", project.dir_name(), file.path)).unwrap_or_else(|_| {});
            header.set_size(file.content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar_builder.append(&header, file.content.as_bytes()).unwrap_or_else(|_| {});
        }
        tar_builder.finish().unwrap_or_else(|_| {});
    }

    let tar_data = tar_buffer.into_inner();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    use std::io::Write;
    encoder.write_all(&tar_data).unwrap_or_else(|_| {});
    let gz_data = encoder.finish().unwrap_or_default();

    let filename = format!("{}.tar.gz", project.dir_name());
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/gzip".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
        ],
        gz_data,
    ).into_response()
}

// ─── Twin-Links: API Inspector Handlers ───

/// GET /admin/projects/:id/api-docs — Swagger UI
async fn admin_swagger_ui_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let project = match load_project(pool, id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    let spec = generate_openapi_spec(&project);
    let html = generate_swagger_ui(&spec);
    Html(html).into_response()
}

/// GET /admin/projects/:id/api-spec.json — OpenAPI JSON spec
async fn admin_openapi_spec_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let project = match load_project(pool, id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    let spec = generate_openapi_spec(&project);
    Json(spec).into_response()
}

// ─── Project Settings Handlers ───

/// GET /admin/projects/:id/settings — إعدادات المشروع (تعديل/حذف)
async fn admin_project_settings_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let project = match load_project(pool, id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(format!(
        r##"{admin_header}
<div class="admin-body">
  <div class="admin-sidebar">
    <a href="/admin"><span class="icon">📊</span> لوحة التحكم</a>
    <a href="/admin/users"><span class="icon">👥</span> المستخدمون</a>
    <a href="/admin/projects" class="active"><span class="icon">📦</span> المشاريع</a>
    <a href="/admin/settings"><span class="icon">⚙️</span> الإعدادات</a>
    <a href="/auth/logout"><span class="icon">🚪</span> خروج</a>
  </div>
  <div class="admin-content">
    <h1 class="page-title">⚙️ إعدادات المشروع</h1>
    <p class="page-subtitle">{name}</p>

    <div class="card" style="margin-bottom: 24px;">
      <div class="card-header"><h3>تعديل بيانات المشروع</h3></div>
      <div class="card-body">
        <form method="POST" action="/admin/projects/{id}/update">
          <div class="form-row">
            <div class="form-group">
              <label>الاسم (PascalCase)</label>
              <input type="text" name="name" value="{proj_name}" required pattern="[A-Z][a-zA-Z0-9]*">
            </div>
            <div class="form-group">
              <label>الاسم المعروض</label>
              <input type="text" name="display_name" value="{display_name}" required>
            </div>
          </div>
          <div class="form-group">
            <label>الوصف</label>
            <textarea name="description" rows="2">{desc}</textarea>
          </div>
          <button type="submit" class="btn btn-primary" style="width: auto;">💾 حفظ</button>
        </form>
      </div>
    </div>

    <div class="card" style="border-color: var(--red);">
      <div class="card-header" style="color: var(--red);"><h3>⚠️ منطقة الخطر</h3></div>
      <div class="card-body">
        <p style="margin-bottom: 16px;">حذف المشروع سيحذف كل الكيانات والإعدادات. لا يمكن التراجع.</p>
        <form method="POST" action="/admin/projects/{id}/delete"
              onsubmit="return confirm('⚠️ هل أنت متأكد؟ سيُحذف المشروع بالكامل!')">
          <button type="submit" class="btn btn-danger" style="width: auto;">🗑️ حذف المشروع نهائيًا</button>
        </form>
      </div>
    </div>
  </div>
</div>"##,
        admin_header = admin_header("إعدادات المشروع", &ctx.user.email, initial),
        name = project.display_name,
        id = id,
        proj_name = project.name,
        display_name = project.display_name,
        desc = project.description.as_deref().unwrap_or(""),
    )).into_response()
}

#[derive(Debug, Deserialize)]
struct UpdateProjectForm {
    name: String,
    display_name: String,
    description: Option<String>,
}

/// POST /admin/projects/:id/update — تحديث بيانات المشروع
async fn admin_update_project_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Form(form): Form<UpdateProjectForm>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let mut project = match load_project(pool, id).await {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "Project not found").into_response(),
    };

    project.name = form.name;
    project.display_name = form.display_name;
    project.description = form.description;
    project.updated_at = chrono::Utc::now();

    let config_json = match project.to_json() {
        Ok(j) => j,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON error: {}", e)).into_response(),
    };

    let _ = sqlx::query("UPDATE novax_projects SET name = $1, display_name = $2, description = $3, config = $4, updated_at = NOW() WHERE id = $5")
        .bind(&project.name)
        .bind(&project.display_name)
        .bind(&project.description)
        .bind(&config_json)
        .bind(id)
        .execute(pool)
        .await;

    Redirect::to(&format!("/admin/projects/{}", id)).into_response()
}

/// POST /admin/projects/:id/delete — حذف المشروع نهائيًا
async fn admin_delete_project_handler(
    axum::Extension(ctx): axum::Extension<AuthContext>,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&ctx) {
        return r;
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "DB unavailable").into_response(),
    };

    let _ = sqlx::query("DELETE FROM novax_projects WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await;

    Redirect::to("/admin/projects").into_response()
}

// ─── Helper Functions ───

async fn load_project(pool: &PgPool, id: Uuid) -> Option<ProjectConfig> {
    let row = sqlx::query("SELECT config FROM novax_projects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()?;

    let row = row?;
    let config_json: String = row.try_get("config").ok()?;
    ProjectConfig::from_json(&config_json).ok()
}

async fn save_project(pool: &PgPool, project: &ProjectConfig) -> Result<(), String> {
    let config_json = project.to_json().map_err(|e| e.to_string())?;
    sqlx::query("UPDATE novax_projects SET config = $1, updated_at = NOW() WHERE id = $2")
        .bind(&config_json)
        .bind(project.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Types ───

#[derive(Debug, Deserialize)]
struct PaginationParams {
    page: Option<u32>,
}

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    name: String,
    is_active: bool,
    is_admin: bool,
    email_verified_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

// ─── Errors ───

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),
    #[error("migration error: {0}")]
    Migration(String),
}

const LANDING_PAGE: &str = r#"<!DOCTYPE html>
<html lang="ar" dir="rtl">
<head>
  <meta charset="UTF-8">
  <title>NovaX</title>
  <style>
    body { font-family: sans-serif; background: #0f0f10; color: #f0f0f2; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; }
    .card { background: #1a1a1c; border: 1px solid #2a2a2d; border-radius: 16px; padding: 60px; text-align: center; max-width: 500px; }
    h1 { color: #c79a3a; margin-bottom: 16px; }
    p { color: #8a8a90; }
    .badge { display: inline-block; padding: 4px 12px; background: rgba(199,154,58,0.2); color: #e8b34c; border-radius: 12px; font-size: 12px; margin-top: 12px; }
  </style>
</head>
<body>
  <div class="card">
    <h1>NovaX v0.4.0</h1>
    <p>Database or Auth not configured.</p>
    <p>Set DATABASE_URL and JWT_SECRET environment variables.</p>
    <span class="badge">Server is running</span>
  </div>
</body>
</html>"#;
