# NovaX

> A next-generation full-stack web platform built entirely in Rust.

[![CI](https://github.com/amir-helal-ali/novax/actions/workflows/ci.yml/badge.svg)](https://github.com/amir-helal-ali/novax/actions/workflows/ci.yml)
[![License: Apache-2.0 OR MIT](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](docker-compose.yml)

**NovaX** is a complete development platform that unifies backend, frontend, runtime, compiler, rendering engine, networking, deployment, observability, and tooling into one coherent ecosystem — written entirely in Rust.

## 🎯 Why NovaX?

- **Single language:** Rust end-to-end. No JavaScript, TypeScript, or JSX.
- **Zero boilerplate:** Compiler generates routes, migrations, tests, docs.
- **Maximum performance:** < 10ms startup, < 1ms P99 latency (targets for v1.0).
- **Secure by default:** SQL injection, XSS, CSRF detected at compile-time.
- **Scales naturally:** Single binary → multi-region, no business logic changes.
- **Built-in observability:** Logging, metrics, tracing — no external services required.
- **Docker-ready:** One command to run the entire platform.

## 📦 Current Status (v0.1.0)

This is the **first scaffold release**. It includes:

| Component | Status | Notes |
|-----------|--------|-------|
| Workspace structure | ✅ Ready | 8 crates organized as Cargo workspace |
| Runtime | ✅ Functional | Built on tokio (native NovaX scheduler planned for v0.2) |
| HTTP server | ✅ Functional | HTTP/1.1 + HTTP/2 via axum (HTTP/3 in v0.3) |
| Router | ✅ Functional | Type-safe routes via axum |
| Storage | ✅ Functional | In-memory backend (PostgreSQL/SQLite/MySQL in v0.2) |
| Observability | ✅ Functional | Structured logging, metrics, health checks |
| Procedural Macros | 🚧 Skeleton | `#[novax::main]`, `#[novax::route]`, `#[novax::entity]` defined but minimal |
| UI DSL | 📋 Planned | Rust DSL → WASM + WebGPU (v0.4) |
| CLI | ✅ Functional | `novax new/build/run/test/serve/doctor/info` |
| Docker | ✅ Ready | Multi-stage Dockerfile, dev + prod compose files |
| Security engine | 📋 Planned | Compile-time SQLi/XSS/CSRF detection (v0.3) |
| Plugin system | 📋 Planned | WASM sandboxed plugins (v0.5) |

See [ROADMAP.md](ROADMAP.md) for the full plan.

## 🚀 Quick Start

### Option 1: Docker (recommended)

```bash
# Clone the repository
git clone https://github.com/amir-helal-ali/novax.git
cd novax

# Run the entire platform with one command
docker compose up
```

Open your browser to **http://localhost:3000**

### Option 2: Local development

```bash
# Prerequisites: Rust 1.85+ (https://rustup.rs)

# Clone
git clone https://github.com/amir-helal-ali/novax.git
cd novax

# Build and run
cargo run --release -p novax-app
```

Open **http://localhost:3000**

### Option 3: Use the NovaX CLI

```bash
# Build the CLI
cargo build --release -p novax-cli

# Run with the CLI
./target/release/novax run --host 0.0.0.0 --port 3000
```

## 🌐 Available Endpoints

Once running, the following endpoints are available:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/` | Dashboard (HTML) |
| `GET` | `/api/health` | System health check (JSON) |
| `GET` | `/api/info` | Application info (JSON) |
| `GET` | `/api/version` | Version string |
| `GET` | `/api/metrics` | Prometheus metrics |
| `GET` | `/static/*` | Static file serving |

### Example responses

```bash
# Health check
$ curl http://localhost:3000/api/health
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 42,
  "subsystems": [
    {"status": "healthy", "subsystem": "runtime", ...},
    {"status": "healthy", "subsystem": "network", ...},
    {"status": "healthy", "subsystem": "router", ...}
  ]
}

# App info
$ curl http://localhost:3000/api/info
{
  "name": "NovaX",
  "version": "0.1.0",
  "description": "A next-generation full-stack web platform built entirely in Rust",
  "features": ["Rust end-to-end", "HTTP/1.1 + HTTP/2", ...]
}
```

## 📁 Project Structure

```
novax/
├── crates/                    # NovaX platform crates
│   ├── novax/                 # Meta-crate (re-exports)
│   ├── novax-macros/          # Procedural macros
│   ├── novax-runtime/         # Async runtime
│   ├── novax-router/          # HTTP routing
│   ├── novax-network/         # HTTP/WS/SSE networking
│   ├── novax-storage/         # Storage abstraction
│   ├── novax-observability/   # Logging, metrics, tracing
│   └── novax-cli/             # `novax` command-line tool
├── apps/
│   └── novax-app/             # Example application
├── static/                    # Static web assets
├── docs/                      # Documentation
├── .github/                   # CI/CD + issue templates
├── Dockerfile                 # Multi-stage Docker build
├── docker-compose.yml         # Development compose
├── docker-compose.production.yml  # Production compose
├── Cargo.toml                 # Workspace manifest
└── README.md
```

## 🛠️ Build & Test

```bash
# Build all crates
cargo build --workspace

# Build in release mode
cargo build --workspace --release

# Run tests
cargo test --workspace

# Run the example app
cargo run -p novax-app

# Run the CLI
cargo run -p novax-cli -- info
cargo run -p novax-cli -- doctor
```

## 🐳 Docker Commands

```bash
# Build and run
docker compose up --build

# Run in background
docker compose up -d

# View logs
docker compose logs -f app

# Stop
docker compose down

# Production deployment
docker compose -f docker-compose.production.yml up -d
```

## 📚 Documentation

- [Architecture Overview](docs/architecture.md)
- [Engineering Specification (PDF)](https://github.com/amir-helal-ali/novax/releases)
- [Roadmap](ROADMAP.md)
- [Contributing Guide](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

Dual-licensed under either:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## 🙏 Acknowledgments

NovaX is built on top of amazing Rust ecosystem projects:
- [tokio](https://tokio.rs) — async runtime
- [axum](https://github.com/tokio-rs/axum) — HTTP framework
- [hyper](https://hyper.rs) — HTTP implementation
- [tower](https://github.com/tower-rs/tower) — service abstraction
- [tracing](https://docs.rs/tracing) — structured logging
- [sqlx](https://github.com/launchbadge/sqlx) — async SQL

Future versions (v0.2+) will progressively replace these dependencies with native NovaX implementations.

## 📬 Contact

- **Repository:** [github.com/amir-helal-ali/novax](https://github.com/amir-helal-ali/novax)
- **Issues:** [github.com/amir-helal-ali/novax/issues](https://github.com/amir-helal-ali/novax/issues)
- **Discussions:** [github.com/amir-helal-ali/novax/discussions](https://github.com/amir-helal-ali/novax/discussions)

---

**Built with Rust** · © 2026 NovaX Contributors
