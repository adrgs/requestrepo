# requestrepo.com

<img src="https://i.imgur.com/RIl2c1C.png" width="420">

[![CI/CD](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml/badge.svg)](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml)

A tool for analyzing HTTP, DNS, and SMTP requests with custom DNS records and response files.

## Features

- **Multi-protocol logging**: Capture HTTP, DNS, and SMTP requests in real-time
- **Custom DNS records**: Create A, AAAA, CNAME, and TXT records for your subdomain
- **Custom response files**: Define custom HTTP responses with headers and body
- **Real-time updates**: WebSocket-based live request streaming
- **Auto-TLS**: Automatic HTTPS certificates via Let's Encrypt (DNS-01 for domains, HTTP-01 for IP addresses)
- **IP Geolocation**: Country detection for incoming requests (via DB-IP)
- **Request sharing**: Share individual requests via secure tokens
- **Admin authentication**: Optional password protection for session creation
- **Agent-ready API**: Drive everything with `curl` from Claude Code, Codex or any agent. Long polling, `llms.txt`, an OpenAPI description and an Agent Skill are served by every instance
- **Abuse reporting**: Rate-limited reports suspend a subdomain at once
- **No external dependencies**: In-memory cache with LRU eviction (no Redis required)

## Quick Start

Run RequestRepo with Docker:

```bash
docker run -d \
  --name requestrepo \
  -p 80:80 -p 443:443 -p 53:53/udp -p 53:53/tcp -p 25:25 \
  -e JWT_SECRET=your-secret-key-min-32-chars \
  -e DOMAIN=yourdomain.com \
  -e SERVER_IP=your.server.ip \
  -e TLS_ENABLED=true \
  -e ACME_EMAIL=admin@yourdomain.com \
  -v requestrepo-certs:/app/certs \
  -v requestrepo-data:/app/data \
  ghcr.io/adrgs/requestrepo:latest
```

This starts all services: HTTP (80), HTTPS (443), DNS (53), SMTP (25). The `certs` volume
keeps the TLS certificates and the `data` volume keeps the abuse blocklist across restarts.

## Using Docker Compose

### Production (Pull from Registry)

```sh
git clone --depth 1 https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env  # Edit .env with your settings

# Use production image from ghcr.io
docker compose -f docker-compose.yml up -d
```

### Development (Build Locally)

```sh
git clone https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env

# Build and run locally (uses docker-compose.override.yml automatically)
docker compose up -d --build
```

This starts all services:
- HTTP server on port 80
- HTTPS server on port 443 (with auto-TLS)
- DNS server on port 53
- SMTP server on port 25

## DNS Configuration

For DNS logging to work, configure your domain's nameserver to point to your server.

### Root Domain Setup

Dedicate the entire domain to requestrepo (e.g., `requestrepo.com`):

| Record Type | Name  | Value             |
| ----------- | ----- | ----------------: |
| NS          | `@`   | `ns1.example.com` |
| A           | `ns1` | `<SERVER_IP>`     |

### Subdomain Setup

Run requestrepo on a subdomain (e.g., `rr.example.com`):

| Record Type | Name  | Value             |
| ----------- | ----- | ----------------: |
| NS          | `rr`  | `ns1.example.com` |
| A           | `ns1` | `<SERVER_IP>`     |

For subdomain setups behind a reverse proxy, ensure:
1. The `Host` header is preserved
2. WebSocket support is enabled for `/api/v2/ws`

## IP Geolocation

The Docker image includes the [DB-IP](https://db-ip.com/db/download/ip-to-country-lite) country database by default. For local development without Docker, download manually:

```sh
mkdir -p ip2country/vendor
curl -o ip2country/vendor/dbip-country-lite.csv.gz \
  "https://download.db-ip.com/free/dbip-country-lite-$(date +%Y-%m).csv.gz"
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) 1.75+
- [Bun](https://bun.sh/) 1.0+
- Docker (optional, for deployment)

### Installation

```sh
make install
```

### Starting Services

```sh
# Start backend (Rust)
make start-backend

# Start frontend (React/Vite)
make start-frontend
```

### Available Commands

| Command | Description |
|---------|-------------|
| `make install` | Install all dependencies and git hooks |
| `make start-backend` | Start Rust backend server |
| `make start-frontend` | Start React development server |
| `make build` | Build Rust backend (release mode) |
| `make test` | Run all tests |
| `make lint` | Run linters (clippy + eslint) |
| `make format` | Format code (rustfmt + prettier) |
| `make docker-build` | Build Docker image |
| `make docker-up` | Start Docker containers |
| `make docker-down` | Stop Docker containers |

## Architecture

```
requestrepo/
├── src/                 # Rust backend
│   ├── cache/          # In-memory LRU cache
│   ├── certs/          # TLS/ACME certificate management
│   ├── dns/            # DNS server
│   ├── http/           # HTTP/HTTPS server + REST API
│   ├── smtp/           # SMTP server
│   └── utils/          # JWT, config, helpers
├── frontend/           # React frontend
│   ├── src/
│   │   ├── components/ # UI components
│   │   ├── pages/      # Page components
│   │   ├── stores/     # Zustand state stores
│   │   └── hooks/      # Custom React hooks
│   └── ...
└── ip2country/         # IP geolocation database
```

## Using requestrepo from AI agents

Every instance documents itself for agents on its main domain, so Claude Code, Codex or any
other agent can use it with `curl` alone: no SDK and no MCP server.

| Path | What it is |
|------|------------|
| `/llms.txt` | Index for language models ([llmstxt.org](https://llmstxt.org)) with a three-call quickstart |
| `/docs/api.md` | The agent guide: sessions, long polling, responses, DNS, errors |
| `/skill.md` | An [Agent Skill](https://agentskills.io); save it as `requestrepo/SKILL.md` in your skills directory |
| `/openapi.json` | OpenAPI 3.1 description of the API |
| `/.well-known/api-catalog` | [RFC 9727](https://www.rfc-editor.org/rfc/rfc9727) API catalog |
| `/robots.txt`, `/sitemap.xml` | With [Content Signals](https://contentsignals.org) |

`/` and `/abuse` also answer `Accept: text/markdown` with the matching guide, and HTML pages
carry `Link` headers pointing at the documents above. These paths are only served on the
main domain; on a session subdomain every path still belongs to the session.

```bash
S=$(curl -s -X POST https://<your-domain>/api/v2/sessions)      # no body needed
TOKEN=$(echo "$S" | jq -r .token); URL=$(echo "$S" | jq -r .url)
curl -s "${URL}callback"
# Long poll: returns as soon as something arrives (or after 60 s)
curl -s -H "Authorization: Bearer $TOKEN" "https://<your-domain>/api/v2/requests?wait=60"
```

Pass the `cursor` of one response as `after` in the next to receive only new requests.
`PUT /api/v2/files/<path>` with `{"body": "..."}` sets a single response without base64.
In the dashboard, the robot button copies a prompt that hands the current session to an agent.

## Reporting Abuse

Anyone can report a subdomain that is used for abuse, such as a malware beacon, command and
control, or a phishing page: with the flag button in the dashboard, at
`https://<your-domain>/abuse`, or over the API:

```bash
curl -X POST https://<your-domain>/api/v2/abuse/reports \
  -H 'Content-Type: application/json' -d '{"subdomain":"abcd1234"}'
```

`subdomain` accepts a label, any hostname under it, or a URL. Nothing else is asked for. Only
a subdomain that is in use (it was handed out as a session, or has received traffic) can be
reported, so reports cannot be used to fill the list with names that never existed.

**A report suspends the subdomain immediately.** Nobody has to be awake for a beacon to stop
working, and the owner of a legitimate subdomain caught by mistake can simply create a new one.
A suspended subdomain answers `410 Gone` (a short page for browsers) and serves nothing, DNS
answers `NXDOMAIN`, mail is refused, and the owner's dashboard, API token and share links stop
working with `subdomain_suspended`. Nothing sent to it is logged any more.

Reports are rate limited to 3 per hour per IP address (an IPv6 /48 counts as one). On a
private instance (`ADMIN_TOKEN` set), reporting needs the admin token like everything else.

**Lifting a suspension.** Suspensions are saved to `abuse.json` in `DATA_DIR` (the `data`
volume in Docker), one per line, and survive restarts. To lift one, delete its line and
restart. A file that cannot be read stops the server from starting instead of silently
dropping suspensions. At 50,000 suspensions (about 2 MB) new reports are refused; nothing is
ever lifted automatically. Checking a request against the list takes about 20 ns.

## Environment Variables

See [.env.example](.env.example) for all available options.

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `JWT_SECRET` | Yes | - | Secret key for JWT signing (min 32 chars) |
| `DOMAIN` | Yes | - | Base domain (e.g., `requestrepo.com`) |
| `SERVER_IP` | Yes | - | Public IP for DNS responses |
| `SERVER_IPV6` | No | - | Public IPv6 address for AAAA responses (IPv4-only when unset) |
| `ADMIN_TOKEN` | No | - | Password for session creation |
| `ABUSE_REPORTING_ENABLED` | No | `true` | Accept abuse reports, which suspend a subdomain immediately |
| `ABUSE_CONTACT` | No | - | Shown to owners of a suspended subdomain, for disputes |
| `DATA_DIR` | No | `./data` | Directory for `abuse.json`, the saved blocklist (`/app/data` in Docker) |
| `TLS_ENABLED` | No | `false` | Enable HTTPS with Let's Encrypt |
| `ACME_EMAIL` | No | - | Email for Let's Encrypt (required if TLS enabled) |
| `IP_CERT_ENABLED` | No | `false` | Enable IP address TLS certificates (HTTP-01) |
| `IP_CERT_CHECK_HOURS` | No | `6` | How often to check IP cert expiry (hours) |
| `IP_CERT_RENEWAL_HOURS` | No | `96` | Renew IP cert when fewer than this many hours remain |
| `HTTP_PORT` | No | `80` | HTTP server port |
| `HTTPS_PORT` | No | `443` | HTTPS server port |
| `DNS_PORT` | No | `53` | DNS server port |
| `SMTP_PORT` | No | `25` | SMTP server port |
| `SENTRY_DSN_BACKEND` | No | - | Sentry DSN for backend error tracking |
| `SENTRY_DSN_FRONTEND` | No | - | Sentry DSN for frontend error tracking |
| `CACHE_MAX_MEMORY_PCT` | No | `0.7` | Max cache memory as % of container limit |
| `MAX_SUBDOMAIN_SIZE_MB` | No | `10` | Max storage per subdomain |
| `MAX_REQUEST_BODY_MB` | No | `10` | Max HTTP request body size |
| `SESSION_RATE_LIMIT` | No | `10` | Max sessions per IP per window |
| `SESSION_RATE_WINDOW_SECS` | No | `60` | Rate limit window in seconds |
| `DANGEROUSLY_ALLOW_SAME_ORIGIN_USER_CONTENT` | No | `false` | Enable `/r/<subdomain>` user content on the dashboard origin (for domainless/IP deployments) |
| `DANGEROUSLY_ALLOW_ALL_HEADERS` | No | `false` | Allow origin-scoped headers such as `Service-Worker-Allowed` and `Alt-Svc` on `/r/` routes |
| `RUST_LOG` | No | `info` | Log level (trace, debug, info, warn, error) |

## Interface

![requestrepo demo](https://i.imgur.com/pzn8O18.png)

## Security Acknowledgments

Thank you to the following researchers for responsibly disclosing security issues:

- [debsec](https://x.com/deb_security) - LFI via improper path handling
- [JaGoTu](https://infosec.exchange/@jagotu) - DoS via unrestricted file upload
- [m0z](https://x.com/LooseSecurity) - LFI via session subdomain
- [Jorian](https://x.com/J0R1AN) - Session hijacking via Service-Worker-Allowed header
- [aisafe.io](https://aisafe.io) - Insecure JWT_SECRET default allowing token forgery

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

If you find this project useful, please consider giving it a star on GitHub.
