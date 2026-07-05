# Contributing to NovaX

First off, thank you for considering contributing to NovaX! 🎉

This document outlines the process for contributing to the NovaX platform.

## 📋 Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md). Please be respectful and inclusive in all interactions.

## 🚀 Getting Started

### Prerequisites

- **Rust** 1.82 or later ([install via rustup](https://rustup.rs))
- **Git**
- **Docker** (optional, for testing container builds)
- **Make** (optional, for convenience commands)

### Setup Development Environment

```bash
# Fork and clone the repository
git clone https://github.com/YOUR_USERNAME/novax.git
cd novax

# Add upstream remote
git remote add upstream https://github.com/amir-helal-ali/novax.git

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run the example app
cargo run -p novax-app
```

### Project Structure

Familiarize yourself with the [architecture overview](docs/architecture.md). NovaX is organized as a Cargo workspace with 8 crates:

- `novax` — meta-crate
- `novax-runtime` — async runtime
- `novax-router` — HTTP routing
- `novax-network` — networking layer
- `novax-storage` — storage abstraction
- `novax-observability` — logging/metrics
- `novax-macros` — procedural macros
- `novax-cli` — CLI tool

## 🛠️ Development Workflow

### 1. Create a Branch

```bash
git checkout main
git pull upstream main
git checkout -b feature/your-feature-name
```

Branch naming conventions:
- `feature/...` — new features
- `fix/...` — bug fixes
- `docs/...` — documentation changes
- `refactor/...` — code refactoring
- `test/...` — test additions/improvements

### 2. Make Your Changes

- Follow the [Rust Style Guide](https://github.com/rust-dev-tools/fmt-rfcs/blob/master/guide/guide.md)
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix any warnings
- Add tests for new functionality
- Update documentation as needed

### 3. Test Your Changes

```bash
# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Tests
cargo test --workspace

# Run the app to verify manually
cargo run -p novax-app
```

### 4. Commit Your Changes

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat` — new feature
- `fix` — bug fix
- `docs` — documentation
- `style` — formatting only
- `refactor` — code refactoring
- `test` — test additions
- `chore` — build/tooling changes

Examples:
```
feat(runtime): add work-stealing scheduler
fix(network): handle connection timeout properly
docs(readme): update installation instructions
test(storage): add memory backend tests
```

### 5. Push and Open a Pull Request

```bash
git push origin feature/your-feature-name
```

Open a PR against `main` and fill out the PR template.

## 📊 Pull Request Guidelines

Before submitting a PR, ensure:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] New tests added for new functionality
- [ ] Documentation updated (rustdoc comments)
- [ ] CHANGELOG.md updated (if applicable)
- [ ] Commit messages follow Conventional Commits

### PR Review Process

1. Automated CI must pass (formatting, clippy, tests)
2. At least one maintainer review required
3. Changes requested should be addressed promptly
4. Squash commits before merging (we squash-merge by default)

## 🏗️ Architecture Guidelines

When contributing, keep these principles in mind:

1. **Modularity:** Each crate should be independently usable
2. **Type safety:** Leverage Rust's type system to prevent bugs at compile-time
3. **Zero-cost abstractions:** Prefer traits with monomorphization
4. **Composition over inheritance:** Use trait composition, not deep hierarchies
5. **Immutability:** Default to immutable data; mutation requires explicit opt-in
6. **Documentation:** All public APIs must have rustdoc comments with examples

## 🧪 Testing

We test at multiple levels:

- **Unit tests:** `#[cfg(test)]` modules in each file
- **Integration tests:** `tests/` directory in each crate
- **Doc tests:** Examples in rustdoc comments
- **End-to-end:** Future, via `tests/e2e/`

Aim for >80% coverage on new code.

## 📝 Documentation

- All public items need rustdoc comments
- Include examples in doc comments where helpful
- Update README.md for user-facing changes
- Update CHANGELOG.md for notable changes

## 🐛 Reporting Bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.yml) when reporting bugs. Include:

- NovaX version
- Rust version
- OS and architecture
- Steps to reproduce
- Expected vs actual behavior
- Logs/screenshots if applicable

## 💡 Suggesting Features

Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.yml) for new feature ideas. Explain:

- The use case
- Why existing solutions don't work
- Proposed API/behavior
- Willingness to implement

## ❓ Questions?

Open a [GitHub Discussion](https://github.com/amir-helal-ali/novax/discussions) for questions.

Thank you for contributing! 🙏
