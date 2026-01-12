# Multi-stage Dockerfile for RequestRepo Rust backend
# Stage 1: Build Rust backend
FROM rust:1.77-alpine AS rust-builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

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
FROM node:20-alpine AS frontend-builder

WORKDIR /app

# Copy package files
COPY frontend/package*.json ./

# Install dependencies
RUN npm ci

# Copy frontend source
COPY frontend/ ./

# Build frontend
RUN npm run build

# Stage 3: Final runtime image
FROM alpine:3.19

RUN apk add --no-cache ca-certificates tzdata

# Create non-root user
RUN addgroup -g 1000 requestrepo && \
    adduser -u 1000 -G requestrepo -s /bin/sh -D requestrepo

WORKDIR /app

# Create certificate directory for Let's Encrypt certs
RUN mkdir -p /app/certs && chown requestrepo:requestrepo /app/certs

# Copy Rust binary
COPY --from=rust-builder /app/target/release/requestrepo /app/requestrepo

# Copy frontend build
COPY --from=frontend-builder /app/build /app/public

# Copy IP2Country database if it exists
COPY --chown=requestrepo:requestrepo ip2country/ /app/ip2country/

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
