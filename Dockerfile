# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# NovaX Dockerfile — Multi-stage build
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# ─── Stage 1: Builder ───
FROM rust:1.82-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests first (better layer caching)
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY apps/ ./apps/
COPY static/ ./static/

# Build the application in release mode
RUN cargo build --release --bin novax-app

# ─── Stage 2: Runtime (minimal image) ───
FROM debian:bookworm-slim AS runtime

# Install only runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -r -s /bin/false -u 1000 novax

# Create app directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/novax-app /app/novax-app

# Copy static assets (for serving)
COPY --from=builder /app/static /app/static

# Make binary executable
RUN chmod +x /app/novax-app

# Switch to non-root user
USER novax

# Expose port
EXPOSE 3000

# Environment defaults
ENV RUST_LOG=info \
    HOST=0.0.0.0 \
    PORT=3000 \
    NOVAX_ENV=production

# Health check
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -sf http://localhost:3000/api/health || exit 1

# Run the application
ENTRYPOINT ["/app/novax-app"]
