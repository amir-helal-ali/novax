# Changelog

All notable changes to NovaX will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- SQLite backend for embedded use cases
- MySQL backend
- HTTP/3 + QUIC transport
- Procedural macros for `#[route]` and `#[entity]` (full code generation)
- UI DSL with WASM + WebGPU rendering
- Compile-time security checks (SQLi/XSS/CSRF detection)
- Plugin system with WASM sandbox
- Hot reload for development

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
