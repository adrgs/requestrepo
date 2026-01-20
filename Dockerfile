# Multi-stage Dockerfile for RequestRepo Rust backend
# Uses cargo-chef for optimized dependency caching

# Stage 1: Chef - install cargo-chef
FROM rust:1.85-slim-bookworm AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
WORKDIR /app

# Stage 2: Planner - analyze dependencies
FROM chef AS planner
COPY src/Cargo.toml src/Cargo.lock ./
COPY src/src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder - build dependencies then source
FROM chef AS rust-builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this layer is cached unless dependencies change
RUN cargo chef cook --release --recipe-path recipe.json
# Copy source and build application
COPY src/Cargo.toml src/Cargo.lock ./
COPY src/src ./src
RUN cargo build --release

# Stage 4: Build frontend
FROM oven/bun:1.2-slim AS frontend-builder

WORKDIR /app

# Copy package files
COPY frontend/package.json frontend/bun.lockb* ./

# Install dependencies
RUN bun install

# Copy frontend source
COPY frontend/ ./

# Build frontend
RUN bun run build

# Stage 5: Download IP geolocation database
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

# Stage 6: Final runtime image
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
