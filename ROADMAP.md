# NovaX Roadmap

This document outlines the development plan for NovaX.

## 🎯 Vision

NovaX aims to be a **production-ready, enterprise-grade platform** that combines the strengths of modern ecosystems (Rust's safety, WASM's portability, modern web standards) while remaining practical and maintainable.

## 📅 Release Timeline

### v0.1.0 — Initial Scaffold ✅ (July 2026)

**Goal:** Establish the foundation and prove the architecture.

- [x] Cargo workspace with 8 crates
- [x] Async runtime (via tokio)
- [x] HTTP/1.1 + HTTP/2 server
- [x] Type-safe routing
- [x] In-memory storage backend
- [x] Observability (logging, metrics, health)
- [x] CLI tool with basic commands
- [x] Docker setup (Dockerfile, compose files)
- [x] Web dashboard
- [x] REST API endpoints
- [x] GitHub CI/CD

### v0.2.0 — Storage & Migrations (Q3 2026)

**Goal:** Production-grade data persistence with PostgreSQL as the primary backend.

- [x] PostgreSQL backend with connection pooling
- [x] ORM with strongly-typed queries (Repository pattern)
- [x] Migration engine with rollback support
- [x] Transaction support
- [ ] Multi-tier caching (L1 in-process, L2 Redis, L3 PostgreSQL)
- [ ] PostgreSQL advanced features: full-text search, JSON columns, listen/notify

> **Note:** SQLite and MySQL backends are not planned. NovaX targets
> PostgreSQL exclusively for relational data, keeping the platform focused
> and leveraging PostgreSQL-specific features (JSONB, arrays, FTS, etc.).

### v0.3.0 — Security & HTTP/3 (Q4 2026)

**Goal:** Production-grade security and modern transports.

- [ ] Compile-time SQL injection detection
- [ ] Compile-time XSS detection (contextual escaper)
- [ ] CSRF protection middleware
- [ ] Secret leakage detection (`#[secret]` type)
- [ ] HTTP/3 + QUIC transport
- [ ] WebSocket support
- [ ] Server-Sent Events (SSE)
- [ ] Authentication capabilities (OAuth2, OIDC, MFA)
- [ ] Authorization engine (RBAC, ABAC)

### v0.4.0 — UI DSL & WASM (Q1 2027)

**Goal:** Build frontend in Rust.

- [ ] UI DSL with procedural macros
- [ ] Widget library (Page, Column, Row, Text, Button, Input, ...)
- [ ] Reactive state management (signals)
- [ ] Retained-mode scene graph renderer
- [ ] WebGPU rendering backend
- [ ] Canvas2D fallback backend
- [ ] WASM compilation target
- [ ] Hot reload for development

### v0.5.0 — Capabilities & Plugins (Q2 2027)

**Goal:** Pre-built features and extensibility.

- [ ] Capability system (Auth, Payments, Notifications, Chat, Search, Analytics, Realtime, Storage)
- [ ] Plugin system with WASM sandbox
- [ ] Plugin SDK and runtime
- [ ] Stable plugin API with versioning
- [ ] Capability-based authorization

### v0.6.0 — Native Runtime (Q3 2027)

**Goal:** Replace tokio with native NovaX runtime.

- [ ] Work-stealing scheduler (native implementation)
- [ ] Lock-free task queues
- [ ] Hierarchical timing wheel
- [ ] Structured concurrency
- [ ] Cancellation tokens
- [ ] Reduced memory footprint (target: < 256 bytes per task)

### v0.7.0 — Observability & Tooling (Q4 2027)

**Goal:** Production-grade observability and developer tooling.

- [ ] Distributed tracing (OpenTelemetry-compatible)
- [ ] Profiling integration
- [ ] Performance timeline visualization
- [ ] Architecture visualization tool
- [ ] Dependency graph analyzer
- [ ] API explorer (interactive)
- [ ] Documentation generator

### v0.8.0 — Distributed Mode (Q1 2028)

**Goal:** Scale to multi-region deployments.

- [ ] Cluster mode (N instances behind load balancer)
- [ ] Distributed event bus (Kafka backend)
- [ ] DB sharding
- [ ] CQRS support
- [ ] Saga pattern for distributed transactions
- [ ] Multi-region active-active

### v1.0.0 — General Availability (Q2 2028)

**Goal:** Production-ready, enterprise-grade platform.

- [ ] Complete documentation
- [ ] 10+ production case studies
- [ ] SOC 2 Type II compliance
- [ ] External security audit
- [ ] Migration guides from Node.js, Django, Go
- [ ] Professional support offering

## 🏗️ Architecture Milestones

### Milestone 0: Foundation ✅
- Workspace, CI, docs skeleton, contribution guidelines

### Milestone 1: Runtime ✅ (basic)
- Async executor (tokio-based), task spawning, basic scheduling

### Milestone 2: Networking ✅ (basic)
- HTTP/1.1, HTTP/2, server lifecycle, request/response handling

### Milestone 3: Compiler basics 🚧
- Procedural macros defined (skeleton only — full code generation in v0.4)

### Milestone 4: Storage & ORM 📋
- Multi-backend storage, ORM, migrations (v0.2)

### Milestone 5: UI DSL & Renderer 📋
- DSL, scene graph, WASM target, WebGPU backend (v0.4)

### Milestone 6: HTTP/3 + QUIC 📋
- Native HTTP/3 transport (v0.3)

### Milestone 7: Security engine 📋
- Lints, secret detection, authZ (v0.3)

### Milestone 8: Capabilities 📋
- Auth, Payments, Notifications, Chat, etc. (v0.5)

### Milestone 9: Observability & Plugins & GA 📋
- Polish, docs, examples, plugin system (v0.5-v0.7)

## 📊 Success Metrics

For v1.0, we target:

- **Startup time:** < 10ms
- **Request P99 latency:** < 1ms (excluding DB I/O)
- **Throughput:** > 500k requests/sec on 8 cores
- **Memory (idle):** < 50MB RSS
- **Binary size:** < 5MB stripped (hello world)
- **WASM bundle:** < 200KB gzipped (UI baseline)
- **Hot reload:** < 100ms
- **Incremental build:** < 500ms

## 🤝 Community Involvement

We welcome community involvement in shaping the roadmap:

- **Feature requests:** [GitHub Discussions](https://github.com/amir-helal-ali/novax/discussions)
- **RFCs:** Major changes go through RFCs (TODO: RFC process)
- **Surveys:** Annual community surveys (starting v0.3)
- **Contributor calls:** Monthly calls (starting v0.2)

## 📅 Release Cadence

- **Minor releases (0.X.0):** Every 3 months
- **Patch releases (0.X.Y):** As needed for bug fixes
- **Pre-release versions:** `0.X.0-rc.Y` for release candidates

## 📝 Note

This roadmap is a living document and may change based on:
- Community feedback
- Technical discoveries
- Resource availability
- Market conditions

Dates are estimates, not commitments. We prioritize correctness and quality over speed.

---

Last updated: 2026-07-05
