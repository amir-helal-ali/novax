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
use novax_web::render::*;
use serde::{Serialize, Deserialize};
use sqlx::PgPool;
use tracing::{info, error, warn};
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

            self.state.db = Some(pool);

            if let Some(auth_config) = self.auth_config.take() {
                let auth = AuthService::new(auth_config);
                self.state.auth = Some(Arc::new(auth));
                info!("Authentication service initialized");
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
        Redirect::to("/auth/login").into_response()
    } else {
        Html(LANDING_PAGE.to_string()).into_response()
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

    // Recent users (5)
    let recent: Vec<UserRow> = sqlx::query_as("SELECT id, email, name, is_active, is_admin, email_verified_at, created_at FROM users ORDER BY created_at DESC LIMIT 5")
        .fetch_all(pool).await.unwrap_or_default();

    let recent_rows = recent.iter().map(|u| {
        let status_badge = if u.is_active { r#"<span class="badge badge-green">نشط</span>"# } else { r#"<span class="badge badge-red">موقوف</span>"# };
        format!(
            r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
            u.name, u.email, status_badge, u.created_at.format("%Y-%m-%d %H:%M")
        )
    }).collect::<String>();

    let stats = DashboardStats {
        total_users: total.0,
        verified_users: verified.0,
        active_users: active.0,
        admin_users: admins.0,
        recent_users_rows: recent_rows,
    };

    let initial = ctx.user.name.chars().next().unwrap_or('U').to_uppercase().next().unwrap_or('U');
    Html(admin_dashboard(&ctx.user.email, initial, &stats)).into_response()
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
