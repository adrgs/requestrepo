# Multi-stage Dockerfile for RequestRepo Rust backend
# Stage 1: Build Rust backend
FROM rust:1.92-slim-bookworm AS rust-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy Cargo files for dependency caching
COPY src/Cargo.toml src/Cargo.lock ./

# Create dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (this layer will be cached)
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY src/src ./src

# Build the actual application
RUN touch src/main.rs && cargo build --release

# Stage 2: Build frontend
FROM oven/bun:1.3.6-slim AS frontend-builder

WORKDIR /app

# Copy package files
COPY frontend/package.json frontend/bun.lockb* ./

# Install dependencies
RUN bun install

# Copy frontend source
COPY frontend/ ./

# Build frontend
RUN bun run build

# Stage 3: Download IP geolocation database
FROM alpine:3.21 AS ip2country-downloader

RUN apk add --no-cache curl

WORKDIR /data

# Download DB-IP country database (free, updated monthly)
# Using current year-month to get the latest version
# Falls back gracefully if download fails
RUN mkdir -p vendor && \
    YEAR_MONTH=$(date +%Y-%m) && \
    curl -fsSL "https://download.db-ip.com/free/dbip-country-lite-${YEAR_MONTH}.csv.gz" \
      -o vendor/dbip-country-lite.csv.gz || \
    echo "Warning: Could not download DB-IP database, IP geolocation will be disabled"

# Stage 4: Final runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -g 1000 requestrepo && \
    useradd -u 1000 -g requestrepo -s /bin/sh -m requestrepo

WORKDIR /app

# Create certificate directory for Let's Encrypt certs
RUN mkdir -p /app/certs && chown requestrepo:requestrepo /app/certs

# Copy Rust binary
COPY --from=rust-builder /app/target/release/requestrepo /app/requestrepo

# Copy frontend build (Vite outputs to dist/)
COPY --from=frontend-builder /app/dist /app/public

# Copy IP2Country database (downloaded in previous stage)
COPY --from=ip2country-downloader --chown=requestrepo:requestrepo /data/ /app/ip2country/

# Set ownership
RUN chown -R requestrepo:requestrepo /app

USER requestrepo

# Environment variables with defaults
ENV RUST_LOG=info
ENV HTTP_PORT=80
ENV HTTPS_PORT=443
ENV DNS_PORT=53
ENV SMTP_PORT=25
ENV DOMAIN=requestrepo.com
ENV TLS_ENABLED=false
ENV CERT_DIR=/app/certs
ENV CERT_RENEWAL_DAYS=7
ENV CERT_CHECK_HOURS=12

# Expose ports
EXPOSE 80 443 53/udp 53/tcp 25

# Define volume for persistent certificate storage
VOLUME /app/certs

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:${HTTP_PORT}/health || exit 1

# Run the application
CMD ["/app/requestrepo"]
