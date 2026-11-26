# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RequestRepo is an HTTP/DNS/SMTP request analysis tool for security researchers and developers. It captures and displays incoming requests in real-time, allowing users to:
- Inspect HTTP requests with full headers, body, and metadata
- Monitor DNS queries to custom subdomains
- Capture SMTP emails sent to the domain
- Define custom HTTP responses with headers/body
- Set custom DNS records (A, AAAA, CNAME, TXT)
- Share individual requests via secure tokens

## Tech Stack

| Component | Technology |
|-----------|------------|
| Backend | Rust (Tokio async runtime) |
| Frontend | React 19 + Vite + TypeScript |
| UI Library | HeroUI (Tailwind-based) |
| State | Zustand |
| Data Fetching | TanStack React Query |
| Editor | Monaco Editor |
| Build | Cargo (Rust) + Bun (Frontend) |
| Container | Docker (multi-stage alpine) |

## Development Commands

```bash
# Installation
make install           # Install all dependencies and git hooks
make install-deps      # Install Cargo + Bun dependencies only

# Development
make start-backend     # Start Rust backend (HTTP/HTTPS/DNS/SMTP)
make dev-backend       # Start with hot reload (cargo watch)
make start-frontend    # Start Vite dev server

# Building
make build             # Build Rust backend in release mode

# Testing
make test              # Run all tests
make test-backend      # Rust tests only (cargo test)
make test-frontend     # Frontend tests only

# Code Quality
make lint              # Run clippy + eslint
make format            # Run cargo fmt + prettier
make lint-rust         # Rust only
make lint-js           # JavaScript only

# Docker
make docker-build      # Build Docker image
make docker-up         # Start with docker-compose
make docker-down       # Stop containers
```

## Architecture

```
requestrepo/
├── src/                    # Rust backend
│   └── src/
│       ├── main.rs         # Entry point, server startup
│       ├── lib.rs          # Library exports
│       ├── cache/          # In-memory LRU cache (compressed)
│       ├── certs/          # TLS/ACME certificate management
│       │   ├── acme.rs     # Let's Encrypt DNS-01 challenge
│       │   ├── tls.rs      # TLS configuration
│       │   └── storage.rs  # Certificate persistence
│       ├── dns/            # DNS server (UDP/TCP)
│       ├── http/           # HTTP server
│       │   ├── routes.rs   # Legacy v1 routes
│       │   ├── routes_v2.rs # API v2 endpoints
│       │   └── websocket.rs # Real-time updates
│       ├── smtp/           # SMTP server
│       ├── tcp/            # Custom TCP port handling
│       ├── ip2country/     # IP geolocation (DB-IP)
│       ├── abuse/          # Abuse reports, suspension sinkhole (public + admin API)
│       ├── agent/          # llms.txt, agent guide, Agent Skill, OpenAPI (docs/ is the source)
│       ├── models/         # Shared data structures
│       ├── utils/          # JWT auth, config
│       └── tests/          # Integration tests
│
├── frontend/               # React SPA
│   └── src/
│       ├── main.tsx        # Entry point
│       ├── App.tsx         # Router setup
│       ├── api/            # API client (axios)
│       ├── components/     # Reusable UI components
│       │   ├── auth/       # Auth overlays
│       │   ├── file-tree/  # Response file editor tree
│       │   ├── layout/     # App shell (Sidebar, Topbar)
│       │   └── ui/         # Generic UI (ContextMenu)
│       ├── features/       # Feature modules
│       │   └── requests/   # Request display components
│       ├── hooks/          # Custom React hooks
│       │   ├── useWebSocket.ts    # WebSocket connection
│       │   ├── useAutoSession.ts  # Auto session creation
│       │   └── useTheme.ts        # Theme switching
│       ├── lib/            # Utilities
│       │   ├── config.ts   # Runtime configuration
│       │   └── fileTree.ts # File tree logic
│       ├── pages/          # Route components
│       │   ├── RequestsPage.tsx
│       │   ├── DnsSettingsPage.tsx
│       │   ├── ResponseEditorPage.tsx
│       │   ├── ReportAbusePage.tsx   # /abuse, public, outside the dashboard
│       │   └── AbuseAdminPage.tsx    # /admin/abuse, suspensions (ABUSE_ADMIN_TOKEN)
│       ├── stores/         # Zustand stores
│       │   ├── authStore.ts
│       │   ├── sessionStore.ts
│       │   ├── requestStore.ts
│       │   ├── blockedStore.ts
│       │   ├── themeStore.ts
│       │   └── uiStore.ts
│       └── types/          # TypeScript definitions
│
├── ip2country/             # Geolocation database (optional)
├── Dockerfile              # Multi-stage build
├── docker-compose.yml      # Container orchestration
└── Makefile                # Development commands
```

## Backend Modules

### Cache (`cache/mod.rs`)
- In-memory LRU cache with gzip compression
- Stores sessions, requests, DNS records, files
- No external dependencies (replaces Redis)
- Configurable max entries and TTL

### Certificates (`certs/`)
- Auto-TLS via Let's Encrypt (production) or self-signed (dev)
- DNS-01 challenge for wildcard domain certificates
- HTTP-01 challenge for IP address certificates (short-lived, 6-day)
- SNI-based dual cert selection: domain cert when SNI present, IP cert when absent
- Automatic renewal before expiration (separate cycles for domain/IP)
- Certificate persistence to disk

### DNS Server (`dns/mod.rs`)
- UDP and TCP listeners
- Custom record support per session: A, AAAA, CNAME, TXT
- Wildcard subdomain matching
- All queries logged to session

### HTTP Server (`http/`)
- Axum-based with Tower middleware
- Serves frontend static files from `/public`
- API v2 routes under `/api/v2/`
- WebSocket endpoint for real-time updates
- Request logging with geolocation

### SMTP Server (`smtp/mod.rs`)
- Basic SMTP implementation
- Captures emails sent to any `*@subdomain.domain`
- Stores sender, recipients, subject, body

### IP Geolocation (`ip2country/mod.rs`)
- Optional DB-IP database integration
- Country code lookup for incoming IPs
- Graceful fallback if database unavailable

### Abuse (`abuse/`)
- A report suspends a subdomain immediately (rate limited: 3/hour per IPv4 address or IPv6
  /48). Being on the list is being suspended; lifting removes the entry. Reports carry only a
  category and an optional 300-char reason: no free-form URLs or contact fields to abuse.
- Suspended = sinkhole: HTTP 410 (`sinkhole_suspended_subdomains`, the outermost middleware in
  `http/mod.rs`; HTML for browsers, text otherwise), DNS NXDOMAIN, SMTP 550, token refused with
  `subdomain_suspended` everywhere, including open long polls (never `invalid_token`: the
  frontend deletes sessions with an invalid token). HTTP and DNS traffic is still recorded (not
  broadcast) so the admin can export the sources. A new ingress point needs `is_blocked()`.
- `list.rs`: `ArcSwap` snapshot, lock-free reads; changes are clone-modify-save-swap under a
  write lock, saved to `DATA_DIR/abuse.json`. At 5,000 entries the oldest report-made
  suspension is lifted; admin suspensions never are.
- `routes.rs`: public `GET /api/v2/abuse`, `POST /api/v2/abuse/reports`; admin
  `/api/v2/abuse/admin/subdomains/...` with `ABUSE_ADMIN_TOKEN` as Bearer only (no cookie).
  None is CORS-enabled and reports must be `application/json`, so no site can make its
  visitors' browsers file reports.

### Agent documents (`agent/`)
- Served by `catch_all` on the main domain only (never on a session subdomain, whose paths
  belong to the session): `/llms.txt`, `/docs/api.md`, `/docs/abuse.md`, `/skill.md`,
  `/openapi.json`, `/.well-known/api-catalog` (RFC 9727), `/robots.txt` (Content Signals),
  `/sitemap.xml`. `/` and `/abuse` answer `Accept: text/markdown`; HTML gets `Link` headers.
- Sources are in `agent/docs/`, rendered once with `{{ORIGIN}}`, `{{DOMAIN}}`, `{{SCHEME}}`.
  **When you change an endpoint, update `docs/api.md` and `docs/openapi.json` too**;
  `agent::tests` checks that placeholders render and `$ref`s resolve.

## API Reference

### REST Endpoints

All endpoints under `/api/v2/` require JWT authentication via `Authorization: Bearer <token>` header.

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/sessions` | Create new session; body optional (returns token, subdomain, domain, url, email) |
| `GET` | `/sessions/share` | Get share token for current session |
| `GET` | `/sessions/shared/{token}` | Access shared session (read-only) |
| `GET` | `/dns` | Get DNS records for session |
| `PUT` | `/dns` | Update DNS records |
| `GET` | `/files` | List response files |
| `PUT` | `/files` | Update response files |
| `GET` | `/files/{path}` | Get single file content |
| `PUT` | `/files/{path}` | Create/replace one file: `{"body"` or `"raw", "status_code"?, "headers"?}` |
| `DELETE` | `/files/{path}` | Delete one file |
| `GET` | `/requests` | List captured requests; `after` (cursor), `wait` (long poll, ≤60 s), `type`, `limit`, `offset` |
| `DELETE` | `/requests` | Delete all requests |
| `GET` | `/requests/{id}` | Get single request |
| `DELETE` | `/requests/{id}` | Delete single request |
| `GET` | `/abuse` | Abuse reporting policy (public) |
| `POST` | `/abuse/reports` | Report a subdomain, which suspends it (public, rate limited, no CORS) |
| `GET` | `/abuse/admin/subdomains` | Suspended subdomains: `q`, `limit`, `offset` (ABUSE_ADMIN_TOKEN) |
| `GET`/`PUT`/`DELETE` | `/abuse/admin/subdomains/{subdomain}` | One suspension / suspend / lift |
| `GET` | `/abuse/admin/subdomains/{subdomain}/requests` | Captured traffic, including while suspended |
| `GET` | `/abuse/admin/subdomains/{subdomain}/sources` | Distinct source IPs |
| `DELETE` | `/abuse/admin/subdomains/{subdomain}/data` | Purge files, DNS records, requests |
| `GET` | `/ws` | WebSocket upgrade |

### WebSocket Protocol

```typescript
// Client → Server
{ cmd: "connect", token: string }
{ cmd: "ping" }
{ cmd: "disconnect" }

// Server → Client
{ cmd: "connected", subdomain: string }
{ cmd: "pong" }
{ cmd: "request", subdomain: string, data: Request }
{ cmd: "requests", subdomain: string, data: Request[] }
{ cmd: "suspended", subdomain: string, category?: string }  // the session was suspended for abuse
{ cmd: "error", code: string, message: string }
```

### Request Object Structure

```typescript
interface Request {
  id: string;
  type: "http" | "dns" | "smtp";
  timestamp: string;        // ISO 8601
  ip: string;
  country?: string;         // 2-letter code
  raw: string;              // Base64 encoded raw data

  // HTTP-specific
  method?: string;
  path?: string;
  headers?: Record<string, string>;
  body?: string;            // Base64 encoded

  // DNS-specific
  query_type?: string;      // A, AAAA, CNAME, TXT, etc.
  query_name?: string;

  // SMTP-specific
  from?: string;
  to?: string[];
  subject?: string;
}
```

## Data Flow

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│   Client    │──────│  Subdomain  │──────│   Backend   │
│  (Browser)  │      │ abc123.dom  │      │   (Rust)    │
└─────────────┘      └─────────────┘      └──────┬──────┘
                                                  │
                          ┌───────────────────────┼───────────────────────┐
                          │                       │                       │
                     ┌────▼────┐            ┌─────▼─────┐           ┌─────▼─────┐
                     │  HTTP   │            │    DNS    │           │   SMTP    │
                     │ Server  │            │  Server   │           │  Server   │
                     └────┬────┘            └─────┬─────┘           └─────┬─────┘
                          │                       │                       │
                          └───────────────────────┼───────────────────────┘
                                                  │
                                           ┌──────▼──────┐
                                           │    Cache    │
                                           │  (LRU+Gzip) │
                                           └──────┬──────┘
                                                  │
                                           ┌──────▼──────┐
                                           │  WebSocket  │
                                           │  Broadcast  │
                                           └──────┬──────┘
                                                  │
                                           ┌──────▼──────┐
                                           │  Frontend   │
                                           │   (React)   │
                                           └─────────────┘
```

1. Requests arrive at `<subdomain>.<domain>` (HTTP/DNS/SMTP)
2. Backend extracts subdomain, validates session exists
3. Request logged to cache with IP geolocation
4. WebSocket broadcasts to connected clients
5. Frontend receives real-time updates via Zustand store

## Environment Variables

### Required

| Variable | Description |
|----------|-------------|
| `JWT_SECRET` | **Required.** Secret key for JWT signing (min 32 chars enforced). Server panics if <32 chars or a known placeholder. |
| `DOMAIN` | Base domain (e.g., `requestrepo.com`) |
| `SERVER_IP` | Public IP for DNS responses |

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_IPV6` | - | Public IPv6 address for AAAA queries with no user-defined record; unset leaves the zone IPv4-only |
| `HTTP_PORT` | `80` | HTTP server port |
| `HTTPS_PORT` | `443` | HTTPS server port |
| `DNS_PORT` | `53` | DNS server port (UDP+TCP) |
| `SMTP_PORT` | `25` | SMTP server port |
| `TLS_ENABLED` | `false` | Enable HTTPS with auto-TLS |
| `ACME_EMAIL` | - | Email for Let's Encrypt registration |
| `CERT_DIR` | `./certs` | Certificate storage directory |
| `IP_CERT_ENABLED` | `false` | Enable IP address TLS certificates (HTTP-01) |
| `IP_CERT_CHECK_HOURS` | `6` | How often to check IP cert expiry |
| `IP_CERT_RENEWAL_HOURS` | `96` | Renew IP cert when fewer than this many hours remain |
| `ADMIN_TOKEN` | - | Require password for session creation |
| `ABUSE_REPORTING_ENABLED` | `true` | Accept public abuse reports (each suspends a subdomain) |
| `ABUSE_ADMIN_TOKEN` | - | Token for the abuse admin page and API (16+ chars) |
| `ABUSE_CONTACT` | - | Contact for reporters and for owners of a suspended subdomain |
| `DATA_DIR` | `./data` | Directory of `abuse.json`, the saved blocklist (`/app/data` in Docker) |
| `SESSION_RATE_LIMIT` | `10` | Max sessions per IP per window |
| `SESSION_RATE_WINDOW_SECS` | `60` | Rate limit window in seconds |
| `RUST_LOG` | `info` | Log level (trace, debug, info, warn, error) |

### Frontend (Build-time)

| Variable | Description |
|----------|-------------|
| `VITE_DOMAIN` | Domain shown in UI (defaults to `DOMAIN`) |
| `VITE_API_URL` | API base URL (defaults to same origin) |

## Git Hooks

Installed via `make install-hooks`:

- **pre-commit**: Runs `make format` and `make lint`
- **pre-push**: Runs `make test` in addition

## Common Tasks

### Adding a new API endpoint

1. Add route in `src/src/http/routes_v2.rs`
2. Add handler function with proper JWT extraction
3. Update cache operations in `src/src/cache/mod.rs` if needed
4. Add TypeScript types in `frontend/src/types/index.ts`
5. Add API call in `frontend/src/api/client.ts`

### Adding a new request type

1. Update `RequestType` enum in `src/src/models/mod.rs`
2. Add capture logic in appropriate server module
3. Update frontend `Request` type in `types/index.ts`
4. Add display component in `features/requests/components/`

### Modifying DNS record types

1. Update `DnsRecordType` in `src/src/models/mod.rs`
2. Update DNS response builder in `src/src/dns/mod.rs`
3. Update frontend form in `pages/DnsSettingsPage.tsx`

### Debugging WebSocket issues

1. Check browser DevTools Network tab for WS connection
2. Enable `RUST_LOG=debug` on backend
3. Verify JWT token is valid and not expired
4. Check WebSocket store in React DevTools (Zustand)

### Common CI/CD gotchas

- **Docker build runs `tsc -b` (strict)** — the Docker `bun run build` step runs TypeScript compilation before Vite build. Local `bun run lint` may pass but Docker build fails on type errors. Always run `bun run tsc --noEmit` locally before pushing.
- **React doesn't type non-standard HTML attributes** — e.g., `csp` on `<iframe>` is valid HTML but not in React's type definitions. Use `<meta http-equiv="Content-Security-Policy">` inside `srcDoc` instead.
- **CI rustfmt may differ from local** — CI uses the latest stable rustfmt. If formatting fails in CI but passes locally, the pre-commit hook may have reformatted differently. Check `git show HEAD:<file>` to see what was actually committed.
- **CI clippy may have newer lints** — e.g., `manual_is_multiple_of` was added in a newer Rust. Use `is_multiple_of()` instead of `% X == 0`.

## Testing

### Backend Tests

```bash
cd src && cargo test
```

Tests are in `src/src/tests/`:
- `cache_tests.rs` - Cache operations
- `dns_tests.rs` - DNS record handling
- `http_tests.rs` - HTTP route tests
- `auth_tests.rs` - JWT validation
- `certs_tests.rs` - Certificate management
- `harness.rs` - Real server on a loopback port plus a bare HTTP/WebSocket client
- `abuse_tests.rs` - Reporting, suspension, sinkhole, over HTTP, WebSocket and DNS
- `agent_api_tests.rs` - Session creation, cursor/long polling, single files, agent documents
- `acme_staging_tests.rs` - ACME integration (requires network)

### Frontend

Currently no test suite. Lint with `bun run lint`, type-check with `bun run tsc --noEmit`.

## Deployment

### Docker (Recommended)

```bash
# Build image
docker build -t requestrepo .

# Run with environment
docker run -d \
  -p 80:80 -p 443:443 -p 53:53/udp -p 53:53/tcp -p 25:25 \
  -e JWT_SECRET=your-secret \
  -e DOMAIN=requestrepo.com \
  -e SERVER_IP=1.2.3.4 \
  -e TLS_ENABLED=true \
  -e ACME_EMAIL=admin@example.com \
  -e IP_CERT_ENABLED=true \
  -v ./certs:/app/certs \
  requestrepo
```

### DNS Setup

For DNS logging to work, configure NS records pointing to your server:

```
# Root domain setup
NS    @     ns1.requestrepo.com
A     ns1   <SERVER_IP>

# Or subdomain setup
NS    rr    ns1.example.com
A     ns1   <SERVER_IP>
```

### Production (requestrepo.com)

Production runs on `130.61.138.67` (ssh as `ubuntu`):

```bash
ssh ubuntu@130.61.138.67
cd /home/ubuntu/requestrepo-prod
docker compose pull && docker compose up -d
docker logs -f requestrepo  # monitor
```

Config: `/home/ubuntu/requestrepo-prod/.env`
Image: `ghcr.io/adrgs/requestrepo:latest` (built by GitHub Actions Release workflow on push to main)
