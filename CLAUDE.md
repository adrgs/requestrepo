# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Installation and Setup
```bash
make install          # Install all dependencies and git hooks
make install-deps     # Install dependencies only (Cargo + npm)
make install-hooks    # Install git hooks only
```

### Starting Services (Development)
```bash
make start-backend    # Start Rust backend (HTTP, DNS, SMTP servers)
make start-frontend   # Start React/Vite dev server
```

### Building
```bash
make build            # Build Rust backend in release mode
```

### Testing
```bash
make test             # Run all tests (backend + frontend)
make test-backend     # Run Rust backend tests only
make test-frontend    # Run frontend tests only (Jest)
```

### Code Quality
```bash
make lint             # Run all linters (cargo clippy + eslint)
make format           # Format all code (cargo fmt + prettier)
make lint-rust        # Rust only
make lint-js          # JavaScript only
make format-rust      # Rust only
make format-js        # JavaScript only
```

### Docker
```bash
make docker-build     # Build Docker images
make docker-up        # Start containers
make docker-down      # Stop containers
```

## Architecture Overview

RequestRepo v2 is an HTTP/DNS/SMTP request analysis tool with a Rust backend.

### Directory Structure
- `src/` - Rust backend (HTTP, DNS, SMTP, TCP servers)
- `frontend/` - React SPA with Vite

### Rust Backend (`src/`)
Modular architecture with the following components:
- `cache/` - In-memory compressed cache with LRU eviction (replaces Redis)
- `certs/` - TLS certificate management with ACME auto-renewal
- `dns/` - DNS server with custom record support (A, AAAA, CNAME, TXT)
- `http/` - HTTP server with REST API and WebSocket
- `smtp/` - SMTP server for email logging
- `tcp/` - Custom TCP ports per session
- `models/` - Data structures for requests, DNS records, files
- `utils/` - JWT auth, config, helpers

### Frontend (`frontend/`)
- React 18 with Vite build system
- Monaco Editor for response customization
- PrimeReact component library
- WebSocket client for real-time request streaming
- **Test location**: `src/__tests__/`

### API Endpoints (v2)

REST API (all under `/api/v2/`):
- `POST /sessions` - Create new session (rate limited)
- `GET/PUT /dns` - Manage DNS records
- `GET/PUT /files` - Manage response files
- `GET /files/{path}` - Get single file
- `GET/DELETE /requests` - List/delete all requests
- `GET/DELETE /requests/{id}` - Get/delete single request
- `GET /ws` - WebSocket for real-time updates

WebSocket Protocol:
```json
// Client → Server
{"cmd": "connect", "token": "jwt..."}
{"cmd": "ping"}
{"cmd": "disconnect"}

// Server → Client
{"cmd": "connected", "subdomain": "abc123"}
{"cmd": "pong"}
{"cmd": "request", "subdomain": "abc123", "data": {...}}
{"cmd": "requests", "subdomain": "abc123", "data": [...]}
{"cmd": "error", "code": "invalid_token", "message": "..."}
```

### Data Flow
1. HTTP/DNS requests arrive at subdomain (e.g., `abc123.domain.com`)
2. Backend extracts subdomain, logs request with geolocation
3. Request broadcast to WebSocket subscribers
4. Frontend receives real-time updates

### Key Environment Variables
```
JWT_SECRET           # Required: JWT signing secret
DOMAIN               # Required: Base domain (e.g., requestrepo.com)
SERVER_IP            # Required: Server's public IP
HTTP_PORT            # Optional: HTTP port (default: 21337)
DNS_PORT             # Optional: DNS port (default: 53)
SMTP_PORT            # Optional: SMTP port (default: 25)
TLS_ENABLED          # Optional: Enable HTTPS (default: false)
HTTPS_PORT           # Optional: HTTPS port (default: 443)
ACME_EMAIL           # Optional: Email for Let's Encrypt
ADMIN_TOKEN          # Optional: Require token for session creation
SESSION_RATE_LIMIT   # Optional: Max sessions per IP per window (default: 10)
SESSION_RATE_WINDOW_SECS  # Optional: Rate limit window (default: 60)
```

### Git Hooks
Pre-commit runs `make format && make lint`. Pre-push adds `make test`.
