# NovaX Architecture

## Overview

NovaX is organized as a Cargo workspace with 8 crates, each providing a specific
capability of the platform. The architecture follows the Hexagonal Architecture
(Ports & Adapters) pattern with dependency inversion.

```
┌─────────────────────────────────────────────────────────────────┐
│                      Application Layer                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                  apps/novax-app                         │    │
│  │              (Example application)                     │    │
│  └────────────────────────┬────────────────────────────────┘    │
└───────────────────────────┼─────────────────────────────────────┘
                            │ depends on
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Platform Layer (NovaX)                     │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                    novax (meta-crate)                   │   │
│   │              (re-exports all components)                │   │
│   └────────────────────────┬────────────────────────────────┘   │
│                            │                                    │
│  ┌─────────┐ ┌──────────┐ ┌─────────┐ ┌───────────────────┐    │
│  │ runtime │ │ network  │ │ router  │ │   observability   │    │
│  │         │ │          │ │         │ │                   │    │
│  │ tokio   │ │ axum     │ │ axum    │ │ tracing + custom  │    │
│  │         │ │ hyper    │ │         │ │                   │    │
│  └─────────┘ └──────────┘ └─────────┘ └───────────────────┘    │
│                                                                │
│  ┌─────────────────┐  ┌──────────┐  ┌─────────────────┐       │
│  │    storage      │  │  macros  │  │      cli        │       │
│  │                 │  │          │  │                 │       │
│  │ memory (v0.1)   │  │ proc-mac │  │ clap-based      │       │
│  │ pg (v0.2)       │  │  #[main] │  │                 │       │
│  │ sqlite (v0.2)   │  │  #[route]│  │                 │       │
│  └─────────────────┘  └──────────┘  └─────────────────┘       │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Breakdown

### `novax` (meta-crate)
The top-level crate that re-exports all other crates. Users only need to depend on `novax` to get the full platform.

### `novax-runtime`
Async runtime abstraction. In v0.1, this is a thin wrapper around tokio. Future versions (v0.6+) will replace this with a native NovaX work-stealing scheduler.

**Key types:**
- `Runtime`, `RuntimeConfig`
- `block_on()`, `spawn()`, `spawn_task()`
- Re-exports from tokio: `Mutex`, `RwLock`, `oneshot`, `mpsc`, `broadcast`, `sleep`, `timeout`

### `novax-network`
HTTP server and transport abstraction. v0.1 supports HTTP/1.1 and HTTP/2 via axum/hyper. Future versions add HTTP/3 (QUIC), WebSocket, SSE.

**Key types:**
- `ServerConfig`, `serve()`
- `ConnectionInfo`, `Protocol`
- `ServerError`

### `novax-router`
HTTP routing with type-safe handlers. Built on axum. Provides middleware defaults (CORS, compression, tracing).

**Key types:**
- `Router`, `AppState`, `RouterConfig`
- `Json`, `Html`, `StatusCode`, `Response`
- Helper functions: `with_defaults()`, `json_response()`, `error_response()`

### `novax-storage`
Storage abstraction with pluggable backends. v0.1 includes in-memory backend. v0.2 adds PostgreSQL, SQLite, MySQL.

**Key types:**
- `Storage` trait (async)
- `StorageConfig`, `BackendKind`
- `StorageError`, `HealthStatus`
- Implementations: `MemoryStorage`

### `novax-observability`
Built-in observability without external services. Provides structured logging, metrics, health checks.

**Key types:**
- `init_logging()` — initialize tracing subscriber
- `Counter`, `Histogram` — metric types
- `REGISTRY` — global metrics registry
- `system_health()`, `SystemHealth`, `HealthStatus`
- Built-in: `HTTP_REQUESTS_TOTAL`, `HTTP_REQUEST_DURATION`

### `novax-macros`
Procedural macros for zero-boilerplate development. v0.1 provides marker macros; v0.4+ will generate full code.

**Macros:**
- `#[novax::main]` — mark app entrypoint
- `#[novax::route(method, path)]` — mark HTTP handler
- `#[novax::entity(table = "...")]` — mark DB entity

### `novax-cli`
Command-line tool `novax` with subcommands.

**Commands:**
- `novax new <name>` — create new project (scaffold creation: TODO v0.2)
- `novax build [--release]` — build the project
- `novax run [--host H] [--port P]` — run dev server
- `novax serve` — run production server
- `novax test [--coverage]` — run tests
- `novax doctor` — check environment
- `novax info` — show version info

## Dependency Strategy

NovaX uses a minimal set of well-established dependencies. Each must be justified:

| Dependency | Why |
|-----------|-----|
| `tokio` | Async runtime (industry standard; will be replaced in v0.6) |
| `axum` | HTTP framework (will be progressively replaced) |
| `hyper` | HTTP/1.1 + HTTP/2 implementation |
| `tower` | Service abstraction (middleware composition) |
| `tower-http` | HTTP middleware (CORS, compression, tracing) |
| `serde` / `serde_json` | Serialization (universal in Rust ecosystem) |
| `tracing` / `tracing-subscriber` | Structured logging |
| `sqlx` | Async SQL (will be wrapped behind NovaX Storage trait) |
| `clap` | CLI parsing |
| `dashmap` | Concurrent hashmap (lock-free reads) |
| `parking_lot` | Faster Mutex/RwLock than std |
| `crossbeam` | Lock-free data structures |
| `uuid`, `chrono` | Common types |
| `thiserror`, `anyhow` | Error handling |

All other dependencies are transitive from these. We use `cargo-deny` (planned) to audit and enforce this.

## Design Decisions

See [ADRs (Architecture Decision Records)](../docs/adrs/) for detailed rationale on key decisions:

1. **Rust as single language** — memory safety, no GC, WASM target
2. **Retained-mode rendering** (planned) — no Virtual DOM
3. **Procedural macros** for code generation — type-safe, IDE-friendly
4. **HTTP/3 default with HTTP/2 fallback** (planned)
5. **WASM plugins** (planned) — sandboxed, cross-platform
6. **Workspace in mono-repo** — atomic changes, parallel compilation

## Module Independence

Each crate can be used independently:
- `novax-storage` can be used without the HTTP server
- `novax-observability` can be used in any Rust project
- `novax-runtime` is a drop-in tokio wrapper

Future versions will publish each crate separately to crates.io.

## Build Configuration

### Release profile
```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
```

### Dev profile
```toml
[profile.dev]
opt-level = 0
debug = true
incremental = true

[profile.dev.package."*"]
opt-level = 2  # optimize dependencies even in dev
```

## Future Architecture (v1.0 target)

```
┌─────────────────────────────────────────────────────────────────┐
│                    NovaX Platform v1.0                          │
│                                                                │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌─────────────────┐  │
│  │compiler │  │ runtime  │  │renderer │  │  networking     │  │
│  │ (macros)│  │(native!) │  │(scene g)│  │(HTTP/3, QUIC)   │  │
│  └─────────┘  └──────────┘  └─────────┘  └─────────────────┘  │
│                                                                │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌─────────────────┐  │
│  │ UI DSL  │  │ security │  │ storage │  │  observability  │  │
│  │  (Rust) │  │ (lints)  │  │ (ORM)   │  │  (traces+OTel)  │  │
│  └─────────┘  └──────────┘  └─────────┘  └─────────────────┘  │
│                                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │             capabilities + plugins (WASM)               │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

The v0.1 scaffold is the foundation; future versions progressively replace dependencies with native implementations while keeping the API stable.
