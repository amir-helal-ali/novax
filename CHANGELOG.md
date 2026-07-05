# Changelog

All notable changes to NovaX will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- HTTP/3 + QUIC transport
- Procedural macros for `#[route]` and `#[entity]` (full code generation)
- UI DSL with WASM + WebGPU rendering
- Compile-time security checks (SQLi/XSS/CSRF detection)
- Plugin system with WASM sandbox
- Hot reload for development
- OAuth2 full implementation (token exchange + user info fetch)
- SMTP email sending (verification + reset links)

## [0.4.0] — 2026-07-05

### Added
- **Rate Limiting** (new crate `novax-rate-limit`)
  - Token bucket algorithm with per-IP tracking
  - Configurable via env vars: `RATE_LIMIT_ENABLED`, `RATE_LIMIT_MAX_REQUESTS`, `RATE_LIMIT_WINDOW_SECONDS`, `RATE_LIMIT_WHITELIST`
  - Automatic bucket cleanup (every 60s)
  - Returns proper 429 with `Retry-After` header
  - `RateLimitConfig::from_env()` helper
  - Whitelist support (default: 127.0.0.1, ::1)
- **Email Verification** (in `novax-auth`)
  - `create_email_verification_token()` — generates 24h token
  - `verify_email()` — verifies token + sets `email_verified_at`
  - Dev mode: shows verification link directly in UI
  - Production: ready for SMTP integration (TODO)
  - New table `email_verification_tokens`
- **Password Reset** (in `novax-auth`)
  - `create_password_reset_token()` — generates 1h token
  - `reset_password()` — verifies token + sets new password + revokes sessions
  - User enumeration prevention (same response regardless of email existence)
  - New table `password_reset_tokens`
- **OAuth2 Scaffolding** (in `novax-auth`)
  - `OAuthConfig` with Google + GitHub support
  - Configurable via env vars: `GOOGLE_OAUTH_CLIENT_ID`, `GOOGLE_OAUTH_CLIENT_SECRET`, `GITHUB_OAUTH_CLIENT_ID`, `GITHUB_OAUTH_CLIENT_SECRET`, `OAUTH_REDIRECT_BASE`
  - `build_auth_url()` for redirecting to provider
  - CSRF state generation (`generate_state()`)
  - New table `oauth_accounts` for linking providers to users
  - Note: token exchange + user info fetch TODO (callback handler shows notice)
- **File Uploads** (avatars)
  - `POST /api/users/avatar` (multipart/form-data, protected)
  - Max 2MB, image types only (png/jpg/jpeg/gif/webp)
  - Stored in `/uploads/{user_id}.{ext}`
  - User's `avatar_url` updated automatically
  - Docker volume `uploads_data` for persistence
- **Auth UI Pages** (new crate `novax-web`)
  - All pages server-rendered HTML (no SPA framework)
  - RTL Arabic with dark theme matching v0.3
  - `/auth/login` — login form with OAuth buttons (if enabled)
  - `/auth/register` — registration form
  - `/auth/forgot-password` — request password reset
  - `/auth/reset-password?token=X` — set new password
  - `/auth/verify-email?token=X` — verify email
  - All forms return helpful error messages in Arabic
  - Cookie-based sessions (HttpOnly, SameSite=Lax)
- **Admin Dashboard** (in `novax-web`)
  - `/admin` — overview with stats (total/verified/active/admin users)
  - `/admin/users` — paginated users list with actions
  - `/admin/settings` — rate limit + OAuth configuration UI
  - Admin-only access (403 for non-admins)
  - User actions: toggle active, toggle admin, delete (with self-protection)
  - Recent users table on dashboard
  - Responsive sidebar navigation
- **New User Fields**
  - `bio` — user bio text
  - `avatar_url` — profile picture URL
  - `is_admin` — admin flag (default: false)
  - `email_verified_at` — verification timestamp
  - First registered user auto-promoted to admin (via DB trigger)

### Removed
- **Posts CRUD system** (per user request — focus on user management)
  - Removed all `/api/posts/*` endpoints
  - Removed migration #002 (create_posts)
  - Posts code removed from `app.rs`
  - Note: existing posts tables (if any) remain in DB but are unused

### Changed
- Bumped workspace version 0.3.0 → 0.4.0
- `App` now supports `.with_rate_limiting()`, `.with_oauth()`, `.dev_mode()`
- `AppState` holds `Option<RateLimiter>` and `Option<OAuthConfig>`
- `AuthUser` now includes: `bio`, `avatar_url`, `is_admin`, `email_verified_at`
- `require_auth` middleware now supports both Bearer token AND cookie-based sessions
- Browser requests without auth → redirect to `/auth/login`
- API requests without auth → 401 JSON error
- `/api/health` now returns `rate_limiting` and `oauth` status
- `/api/info` updated with new features list
- `docker-compose.yml` adds rate limit + OAuth env vars and `uploads_data` volume
- `Dockerfile` creates `/app/uploads` directory with proper ownership
- Auth endpoints moved from `/auth/*` (JSON API) to `/auth/*` (HTML UI) + `/api/auth/*` (JSON API)

### Security
- Rate limiting protects all endpoints from abuse
- Email verification required for new accounts (dev mode auto-shows link)
- Password reset tokens expire in 1 hour
- Email verification tokens expire in 24 hours
- Used tokens cannot be reused
- Password reset revokes all existing sessions
- Admin self-protection: cannot delete self, cannot remove own admin status
- Cookie-based sessions use HttpOnly + SameSite=Lax
- OAuth state parameter for CSRF protection

## [0.3.0] — 2026-07-05

### Added
- **Authentication crate (`novax-auth`)** — full auth system
  - Argon2id password hashing (industry standard, memory-hard)
  - JWT tokens (HMAC-SHA256) with constant-time signature comparison
  - Refresh tokens stored in PostgreSQL for revocation
  - `AuthService` with: `register`, `login`, `logout`, `user_from_token`, `change_password`
  - Password strength validation (min 8 chars)
  - Session table with `revoked_at` for soft revocation
  - User enumeration prevention (same error for wrong email/password)
- **Auth API endpoints**
  - `POST /auth/register` — create a new account (public)
  - `POST /auth/login` — authenticate and receive JWT (public)
  - `GET /auth/me` — get current user (protected)
  - `POST /auth/logout` — revoke all sessions (protected)
  - `POST /auth/change-password` — change password with current verification (protected)
- **Auth middleware** (`require_auth`)
  - Extracts Bearer token from Authorization header
  - Verifies JWT signature and expiration
  - Loads user from DB and injects `AuthContext` into request extensions
  - Returns proper 401 errors for missing/invalid/expired tokens
- **Posts CRUD API** — full REST with FK relations
  - `GET /api/posts?page=1&per_page=20` — paginated list
  - `POST /api/posts` — create (with author_id FK validation)
  - `GET /api/posts/:id` — fetch (auto-increments view_count)
  - `PATCH /api/posts/:id` — update (auto-sets published_at on first publish)
  - `DELETE /api/posts/:id` — delete
  - `GET /api/posts/count` — total count
  - `GET /api/users/:id/posts` — list posts by a specific user
- **Migration #003**: auth_sessions table + password_hash column
  - `auth_sessions` table with refresh_token, expires_at, revoked_at
  - `password_hash` column added to users table
- **Database exclusivity**: PostgreSQL is now the only supported relational backend.
  SQLite and MySQL are no longer planned (see ROADMAP.md).
- **Dashboard enhancements**
  - Auth status indicator (enabled/disabled)
  - Updated features list with auth + posts
  - Updated API endpoints list

### Changed
- Bumped workspace version 0.2.2 → 0.3.0
- `App` now supports `.with_auth(AuthConfig)` for auth configuration
- `AppState` holds `Option<Arc<AuthService>>` for optional auth
- `/api/health` now returns `auth: "enabled"|"disabled"`
- `/api/info` now includes `auth_enabled` flag and updated features list
- `docker-compose.yml` now passes `JWT_SECRET` environment variable
- Migration #001 unchanged (preserves backwards compatibility)

### Security
- **Argon2id** for password hashing (resistant to GPU/ASIC attacks)
- **Constant-time** password and token signature comparison (prevents timing attacks)
- **JWT tokens** with expiration (1h access, 30d refresh)
- **Session revocation** via DB-backed refresh tokens
- **No user enumeration** (login returns same error for wrong email/password)
- **Password strength** validation enforced at registration

## [0.2.2] — 2026-07-05

### Fixed
- **Migration failure: "cannot insert multiple commands into a prepared statement"**.
  PostgreSQL via `sqlx::query` does not support multi-statement prepared statements
  (e.g. `CREATE TABLE ... ; CREATE INDEX ... ; CREATE INDEX ...`).
  Migrations now use `sqlx::raw_sql()` which sends the SQL as a simple query
  string, allowing multiple statements in a single migration file.

  Symptom in logs:
  ```
  ERROR novax::app: Migration failed: database error: migration create_users v1
  failed: cannot insert multiple commands into a prepared statement
  ```

  After this fix, migrations apply successfully and the database is initialized
  on startup, enabling the `/api/users/*` endpoints.

## [0.2.1] — 2026-07-05

### Fixed
- **Docker build failure**: bumped Rust from 1.85 to 1.88 in Dockerfile.
  Recent versions of `home`, `icu_collections`, `icu_locale_core`, `icu_normalizer`,
  `icu_properties`, `icu_provider`, and `idna_adapter` require Rust 1.86+ (home: 1.88+).
- Updated `rust-version` in workspace Cargo.toml to 1.88.
- Updated CI workflow to pin Rust 1.88.
- Updated README to reflect Rust 1.88+ requirement.

## [0.2.0] — 2026-07-05

### Added
- **PostgreSQL storage backend** (`novax-storage` with `postgres` feature)
  - Connection pooling via sqlx
  - KV-store table for `Storage` trait implementation
  - TTL support, automatic expired entry cleanup
  - Health checks
- **NovaX ORM** (`novax-orm` crate)
  - `Entity` trait for strongly-typed models
  - `Repository<T>` with CRUD operations: `find_by_id`, `find_all`, `find_paginated`,
    `count`, `exists`, `delete`, `begin`
  - `Pagination` and `PaginatedResult<T>` types
  - Transactional operations via `with_transaction`
- **NovaX Migration Engine** (`novax-migrate` crate)
  - SQL migration files with `-- +migrate Up` / `-- +migrate Down` markers
  - Versioned migration tracking via `_novax_migrations` table
  - Atomic migrations (each in a transaction)
  - Rollback support (`rollback_last`)
  - Destructive operation detection (DROP TABLE, TRUNCATE, etc.)
  - `load_from_dir` for loading migrations from a directory
- **Database integration in `novax-app`**
  - Connects to PostgreSQL via `DATABASE_URL` env var
  - Auto-runs migrations on startup from `./migrations` directory
  - Graceful degradation: continues without DB if connection fails
- **Users CRUD API** (full REST example)
  - `GET /api/users?page=1&per_page=20` — paginated list
  - `POST /api/users` — create (with email validation)
  - `GET /api/users/:id` — fetch by UUID
  - `PATCH /api/users/:id` — partial update
  - `DELETE /api/users/:id` — remove
  - `GET /api/users/count` — total count
- **Initial migrations**
  - `001_create_users.sql` — users table with email, name, bio, avatar, timestamps
  - `002_create_posts.sql` — posts table with author FK, slug, body, published state
- **PostgreSQL service in docker-compose.yml**
  - postgres:16-alpine with healthcheck
  - Persistent volume `postgres_data`
  - Port 5433 on host (avoids conflict with existing services on 5432)
- **Dashboard enhancements**
  - Live DB status indicator (healthy/unhealthy/disabled)
  - Live users count from database
  - Updated API endpoints list with all CRUD routes
  - Method badges for PATCH/DELETE with color coding

### Changed
- Bumped workspace version 0.1.1 → 0.2.0
- `AppState` now holds `Option<PgPool>` for optional database access
- `App::new()` no longer automatically serves — use `App::new().with_database(cfg).initialize().await`
- `/api/health` now returns database status alongside system health
- `/api/info` now includes `database_enabled` flag and updated features list
- Added `sqlx` workspace dependency (with `postgres`, `uuid`, `chrono`, `macros`, `migrate` features)
- Added `novax-orm` and `novax-migrate` to workspace members
- README updated with v0.2 endpoints and features

### Migration Guide (v0.1.x → v0.2.0)

**Breaking change:** `App::new()` no longer auto-connects to DB.
To use the database:

```rust
// Before (v0.1.x)
let app = App::new();
app.serve("0.0.0.0:3000").await?;

// After (v0.2.0)
let app = App::new()
    .with_database(db_config)
    .initialize()
    .await?;
app.serve("0.0.0.0:3000").await?;
```

If you don't need a database, `App::new()` still works (user endpoints return 503).

## [0.1.1] — 2026-07-05

### Fixed
- **Docker build failure**: bumped Rust from 1.82 to 1.85 in Dockerfile.
  Recent versions of `clap_lex` require Rust edition 2024, stabilized in Rust 1.85.
- **Docker build failure (v0.1.1 regression)**: replaced `--frozen` with `--locked`
  in Dockerfile. `--frozen` blocks all network access (including downloading crates
  from crates.io), causing the build to fail. `--locked` enforces the `Cargo.lock`
  version pinning while still allowing crate downloads.
- Removed obsolete `version: "3.9"` from `docker-compose.yml` and
  `docker-compose.production.yml` (silences Docker Compose warning).
- Updated CI workflow to pin Rust 1.85.
- Updated README to reflect Rust 1.85+ requirement.

### Changed
- Added `rust-version = "1.85"` to workspace `Cargo.toml` for clear error messages
  when building with an older toolchain.
- Docker build now uses `cargo build --locked` to ensure `Cargo.lock` is respected
  in CI/production builds while allowing crate downloads.

## [0.1.0] — 2026-07-05

### Added
- **Initial scaffold release** of the NovaX platform
- Cargo workspace with 8 crates:
  - `novax` — meta-crate re-exporting all components
  - `novax-runtime` — async runtime (built on tokio)
  - `novax-router` — HTTP routing (built on axum)
  - `novax-network` — HTTP/1.1 + HTTP/2 server
  - `novax-storage` — storage abstraction with in-memory backend
  - `novax-observability` — structured logging, metrics, health checks
  - `novax-macros` — procedural macros (`#[novax::main]`, `#[novax::route]`, `#[novax::entity]`)
  - `novax-cli` — `novax` command-line tool
- Example application (`apps/novax-app`) demonstrating platform usage
- Multi-stage Dockerfile with optimized production image
- `docker-compose.yml` for development
- `docker-compose.production.yml` for production deployment
- Web dashboard served at `/` with Arabic RTL UI
- REST API endpoints:
  - `GET /api/health` — system health check
  - `GET /api/info` — application information
  - `GET /api/version` — version string
  - `GET /api/metrics` — Prometheus metrics
- Static file serving at `/static/*`
- Built-in observability: tracing, structured logging (JSON + compact), metrics registry
- CLI commands: `new`, `build`, `run`, `serve`, `test`, `doctor`, `info`
- GitHub Actions CI workflow (formatting, clippy, tests, Docker build)
- Issue and PR templates
- Documentation: README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, ROADMAP
- Dual license: Apache-2.0 OR MIT

### Architecture
- Modular workspace structure with each crate independently replaceable
- Type-safe routing via axum
- Async I/O via tokio multi-threaded runtime
- Configuration via environment variables (12-factor app)
- Health checks for Docker integration

### Known Limitations
- Storage backend is in-memory only (no PostgreSQL/SQLite/MySQL yet)
- Procedural macros are markers only (no code generation yet)
- No HTTP/3 or WebSocket support yet
- No UI DSL or WASM frontend yet
- No plugin system yet

## Versioning

NovaX follows Semantic Versioning:
- **MAJOR** (X.0.0): Breaking changes
- **MINOR** (0.X.0): New features, backward compatible
- **PATCH** (0.0.X): Bug fixes, backward compatible

Until v1.0, breaking changes may occur in minor versions.
