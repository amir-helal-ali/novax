# Changelog

All notable changes to NovaX will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- Native NovaX async runtime (replace tokio dependency)
- PostgreSQL, SQLite, MySQL storage backends
- HTTP/3 + QUIC transport
- Procedural macros for `#[route]` and `#[entity]` (full code generation)
- UI DSL with WASM + WebGPU rendering
- Compile-time security checks (SQLi/XSS/CSRF detection)
- Plugin system with WASM sandbox
- Hot reload for development

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
