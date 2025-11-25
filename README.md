# requestrepo.com

<img src="https://rasp.go.ro/reqlogo.svg" width="420">

[![CI/CD](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml/badge.svg)](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml)

A tool for analyzing HTTP and DNS requests and creating custom DNS records for your subdomain.

## Getting Started

Quick-start using Git and Docker Compose:

```sh
git clone https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env # modify .env as needed
docker compose up --build
```

> **Note:** Remember to edit `.env` before starting the services.

This will set up a production-ready environment with the following services:

- HTTP/S server on ports 80 and 443
- DNS server on port 53

For DNS logging to work, the public IP of the DNS service must be configured as the authoritative nameserver for the domain. Create an `A` record pointing to your server, then add an `NS` record pointing to that `A` record.

Both configurations (root domain and subdomain) are explained below.

### Root Domain Setup

This configuration dedicates the entire domain to requestrepo (as used by https://requestrepo.com). Generated subdomains will be under `*.example.com`.

| Record Type | Name  | Value             |
| ----------- | ----- | ----------------: |
| NS          | `@`   | `ns1.example.com` |
| A           | `ns1` | `<PUBLIC_IP>`     |

An HTTPS certificate will be automatically generated if the `.env` file is configured correctly.

### Subdomain Setup

If you have other applications running on your domain and want to serve requestrepo on a subdomain (e.g., `rr.example.com`), use the following configuration:

| Record Type | Name  | Value             |
| ----------- | ----- | ----------------: |
| NS          | `rr`  | `ns1.example.com` |
| A           | `ns1` | `<PUBLIC_IP>`     |

You can use a reverse proxy such as [Nginx](https://nginx.org/) to route requests based on hostname. Update the port mapping in [docker-compose.yml](docker-compose.yml) to use an internal port:

```yml
    ports:
      - 127.0.0.1:8000:80
```

Then configure your reverse proxy to forward requests to the chosen port. Ensure the following requirements are met:

1. The `Host` header must be preserved for subdomain extraction.
2. WebSocket support must be enabled for `/api/ws` and `/api/ws2` via the `Upgrade` and `Connection` headers.

Example Nginx configuration:

```conf
server {
        listen 80;
        listen [::]:80;
        server_name rr.example.com *.rr.example.com;

        location / {
                proxy_pass http://127.0.0.1:8000;
                proxy_set_header Host $host;
        }

        location ~ ^/api/ws2?$ {
                proxy_pass http://127.0.0.1:8000;
                proxy_http_version 1.1;
                proxy_set_header Upgrade $http_upgrade;
                proxy_set_header Connection "Upgrade";
                proxy_set_header Host $host;
        }
}
```

For HTTPS, generate a certificate at the reverse proxy level rather than through requestrepo. Use [Certbot](https://certbot.eff.org/) to obtain a free wildcard certificate:

```bash
certbot certonly --manual --preferred-challenges dns \
  -d rr.example.com \
  -d "*.rr.example.com"
```

Then configure your reverse proxy to use the certificate:

```conf
listen 443 ssl;
ssl_certificate /etc/letsencrypt/live/rr.example.com/fullchain.pem;
ssl_certificate_key /etc/letsencrypt/live/rr.example.com/privkey.pem;
```

### Enabling IP-to-Country Feature

To enable IP geolocation, download the free IP to Country Lite database from [DB-IP](https://db-ip.com/db/download/ip-to-country-lite).

Place the CSV file at `ip2country/vendor/dbip-country-lite.csv.gz`.

## Development

For development, use the Makefile to manage services.

### Starting Services

```sh
# Start the backend service
make start-backend

# Start the frontend service
make start-frontend
```

### Starting the DNS Server

The DNS server must be started manually:

```sh
cd dns; python ns.py
```

## Interface

![requestrepo demo](https://i.imgur.com/pzn8O18.png)

## Security Acknowledgments

Thank you to the following researchers for responsibly disclosing security issues:

- [debsec](https://x.com/deb_security) — LFI via improper path handling
- [JaGoTu](https://infosec.exchange/@jagotu) — DoS via unrestricted file upload
- [m0z](https://x.com/LooseSecurity) — LFI via session subdomain
- [Jorian](https://x.com/J0R1AN) — Session hijacking via Service-Worker-Allowed header

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## Development Setup

### Prerequisites

- Python 3.11+
- Node.js 18+
- Poetry
- Docker (for Redis)

### Installation

To set up the development environment:

```bash
make install
```

This will:

1. Install backend dependencies using Poetry
2. Install frontend dependencies using npm
3. Set up Git hooks for code quality checks

### Git Hooks

The following hooks are installed automatically:

- **pre-commit**: Runs formatting and linting checks before each commit
- **pre-push**: Runs formatting, linting, and tests before pushing

### Available Commands

| Command | Description |
| ------- | ----------- |
| `make install` | Install dependencies and Git hooks |
| `make install-deps` | Install dependencies only |
| `make install-hooks` | Install Git hooks only |
| `make start-backend` | Start the backend server |
| `make start-frontend` | Start the frontend application |
| `make start-redis` | Start the Redis container |
| `make stop-redis` | Stop the Redis container |
| `make test` | Run all tests |
| `make lint` | Run all linters |
| `make format` | Format all code |

### GitHub Actions

This repository uses GitHub Actions for continuous integration:

1. **Quality Checks** — Runs on push to `main` and on pull requests:
   - Code formatting checks
   - Code linting
   - Tests

2. **Pull Request Checks** — Runs on pull requests to `main`:
   - Combined job for formatting, linting, and testing

These workflows ensure all merged code meets quality standards.

## License

If you find this project useful, please consider giving it a star on GitHub.
