# requestrepo.com

<img src="https://rasp.go.ro/reqlogo.svg" width="420">

[![CI/CD](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml/badge.svg)](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml)

A tool for analyzing HTTP, DNS, and SMTP requests with custom DNS records and response files.

## Features

- **Multi-protocol logging**: Capture HTTP, DNS, and SMTP requests in real-time
- **Custom DNS records**: Create A, AAAA, CNAME, and TXT records for your subdomain
- **Custom response files**: Define custom HTTP responses with headers and body
- **Real-time updates**: WebSocket-based live request streaming
- **Auto-TLS**: Automatic HTTPS certificates via Let's Encrypt (DNS-01 challenge)
- **IP Geolocation**: Country detection for incoming requests (via DB-IP)
- **Request sharing**: Share individual requests via secure tokens
- **Admin authentication**: Optional password protection for session creation
- **No external dependencies**: In-memory cache with LRU eviction (no Redis required)

## Getting Started

Quick-start using Docker Compose:

```sh
git clone https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env  # Edit .env with your settings
docker compose up --build
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

To enable country detection, download the free [DB-IP](https://db-ip.com/db/download/ip-to-country-lite) database:

```sh
mkdir -p ip2country/vendor
curl -o ip2country/vendor/dbip-country-lite.csv.gz \
  https://download.db-ip.com/free/dbip-country-lite-2024-01.csv.gz
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

## Environment Variables

See [.env.example](.env.example) for all available options.

### Required

| Variable | Description |
|----------|-------------|
| `JWT_SECRET` | Secret key for JWT signing |
| `DOMAIN` | Base domain (e.g., `requestrepo.com`) |
| `SERVER_IP` | Public IP address of the server |

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| `ADMIN_TOKEN` | - | Password for session creation |
| `TLS_ENABLED` | `false` | Enable HTTPS with auto-TLS |
| `ACME_EMAIL` | - | Email for Let's Encrypt |
| `HTTP_PORT` | `80` | HTTP server port |
| `HTTPS_PORT` | `443` | HTTPS server port |
| `DNS_PORT` | `53` | DNS server port |
| `SMTP_PORT` | `25` | SMTP server port |

## Interface

![requestrepo demo](https://i.imgur.com/pzn8O18.png)

## Security Acknowledgments

Thank you to the following researchers for responsibly disclosing security issues:

- [debsec](https://x.com/deb_security) - LFI via improper path handling
- [JaGoTu](https://infosec.exchange/@jagotu) - DoS via unrestricted file upload
- [m0z](https://x.com/LooseSecurity) - LFI via session subdomain
- [Jorian](https://x.com/J0R1AN) - Session hijacking via Service-Worker-Allowed header

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

If you find this project useful, please consider giving it a star on GitHub.
