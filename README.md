# 🚀 requestrepo.com

<img src="https://rasp.go.ro/reqlogo.svg" width="420">

[![CI/CD](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml/badge.svg)](https://github.com/adrgs/requestrepo/actions/workflows/quality-checks.yml)

## DNS Configuration and HTTP/DNS Request Analysis Guide

### 🏁 Getting Started

### Step 1: Setting Up Your Domain DNS Records
If your domain registrar doesn't support adding IP addresses directly to your main domain for DNS interactions, you can use services like:

- [sslip.io](https://sslip.io)
- [traefik.me](https://traefik.me)

#### Configure the following DNS records:
```
A record: *.example.com -> <your-ip>
A record: example.com -> <your-ip>
NS record: *.example.com -> www.<your-ip-over-dash>.sslip.io   # For example www.52-0-56-137.sslip.io for 52.0.56.137
NS record: example.com -> www.<your-ip-over-dash>.sslip.io
```
**Note:** Replace `<your-ip>` with your actual IP address (e.g., `192-168-1-1`).

---

### Step 2: Freeing Port 53 on Linux

#### Edit the systemd-resolved configuration file:
```bash
sudo nano /etc/systemd/resolved.conf
```

#### Replace or add the following configuration:
```
DNS=1.1.1.1#cloudflare-dns.com 1.0.0.1#cloudflare-dns.com 2606:4700:4700::1111#cloudflare-dns.com 2606:4700:4700::1001#cloudflare-dns.com
FallbackDNS=8.8.8.8#dns.google 8.8.4.4#dns.google 2001:4860:4860::8888#dns.google 2001:4860:4860::8844#dns.google
DNSStubListener=no
DNSStubListenerExtra=127.0.0.54:54
DNSStubListenerPort=54
```

#### Restart the systemd-resolved service:
```bash
sudo systemctl restart systemd-resolved
```

#### To maintain hostname resolution for internal tools, add your server to `/etc/hosts`:
```bash
echo "127.0.0.1 server-hostname" | sudo tee -a /etc/hosts
```

---

### Step 3: Setting Up Config

#### Create forders:
```bash
mkdir -p requestrepo/config/cert requestrepo/config/vendor && cd requestrepo
```
#### Add Certificates and Country Flags:
To activate HTTPS move or copy your domains `fullchain.pem` and `privkey.pem` to `/config/certs/`**

#### Go to [db-ip](https://db-ip.com/db/download/ip-to-country-lite) download page and download the latest ip-to-country-lite-xxxx-xx.csv.gz file:
Rename to dbip-country-lite-xxxx-xx.csv.gz to dbip-country-lite.csv.gz and paste to /config/vendor/

---
### Step 4: Setting Up Docker Compose
#### Download latest docker-compose.yaml: 
Download `docker-compose.yaml`:
```bash
wget -O docker-compose.yaml https://raw.githubusercontent.com/adrgs/requestrepo/refs/heads/main/docker-compose-prod.yml
```
#### Replace environment variables on `docker-compose.yaml` for backend and dns:
```yaml
      - JWT_SECRET=changethis
      - DOMAIN=example.com
      - SERVER_IP=130.61.138.67
      - TXT=Hello!
      - INCLUDE_SERVER_DOMAIN=true
      - SUBDOMAIN_ALPHABET=abcdefghijklmnopqrstuvwxyz0123456789
      - SUBDOMAIN_LENGTH=8
      - REDIS_HOST=requestrepo_redisdb
      - REDIS_TTL_DAYS=7
```
---
#### Step 5: Start the containers
```bash
docker-compose up -d
```


## 🏁 Getting Started - Build

Quick-start using git and docker compose:

```sh
git clone https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env # modify .env as needed
docker compose up --build
```

⚠️ **Don't forget to edit .env**

This will setup a production-ready environment with the following services:
 - 🌐 HTTP/S server on ports 80 and 443
 - 🔌 DNS server on port 53


## 🔧 Setting up DNS nameservers

In order for DNS logging to work, the public IP of the DNS service must be set up as the authoritative nameserver for the domain.

If the domain registrar does not allow setting up IPs directly as nameservers, a workaround is to use a service like [traefik.me](https://traefik.me/)

## 🌍 Enabling ip2country feature

To enable the ip2country feature, you need to download the free IP to Country Lite database from [db-ip](https://db-ip.com/db/download/ip-to-country-lite).

The csv file must be placed in `ip2country/vendor/dbip-country-lite.csv.gz`

## 💻 Development

For development, it is recommended to use the Makefile to start the services for the best developer experience.

### 🚀 Starting the Services

```sh
# Start the backend service
make start-backend

# Start the frontend service
make start-frontend
```

### 🔌 Starting the DNS Server

The DNS server needs to be started manually:

```sh
# Start the DNS server
cd dns; python ns.py
```

## 🖥️ Interface

![requestrepo demo](https://i.imgur.com/pzn8O18.png)

## 🏆 Hall of Fame

Thank you for reporting a security issue in requestrepo:

- [debsec](https://x.com/deb_security) - 🔒 LFI via improper path handling
- [JaGoTu](https://infosec.exchange/@jagotu) - 🛡️ DoS via unrestricted file upload
- [m0z](https://x.com/LooseSecurity) - 🔐 LFI via session subdomain

## 🤝 Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process for submitting pull requests.

## 🛠️ Development Setup

### 📋 Prerequisites

- 🐍 Python 3.11+
- 📦 Node.js 18+
- 📜 Poetry
- 🐳 Docker (for Redis)

### ⚙️ Installation

To set up the development environment, run:

```bash
make install
```

This will:
1. 📥 Install backend dependencies using Poetry
2. 📦 Install frontend dependencies using npm
3. 🔒 Set up git hooks for code quality checks

### 🔗 Git Hooks

The following git hooks are installed automatically:

- **pre-commit**: ✨ Runs formatting and linting checks before each commit
- **pre-push**: 🧪 Runs formatting, linting, and tests before pushing to remote

These hooks ensure that your code meets quality standards before being shared with others.

### 📝 Available Commands

- `make install` - 🔧 Install dependencies and git hooks
- `make install-deps` - 📦 Install dependencies only
- `make install-hooks` - 🔗 Install git hooks only
- `make start-backend` - 🚀 Start the backend server
- `make start-frontend` - 🖥️ Start the frontend application
- `make start-redis` - 💾 Start the Redis container
- `make stop-redis` - 🛑 Stop the Redis container
- `make test` - 🧪 Run all tests
- `make lint` - 🔍 Run all linters
- `make format` - ✨ Format all code

### 🔄 GitHub Actions

This repository uses GitHub Actions for continuous integration:

1. **Quality Checks** - 🚦 Runs on push to main and pull requests:
   - ✨ Code formatting checks
   - 🔍 Code linting
   - 🧪 Tests

2. **Pull Request Checks** - 🔄 Runs on pull requests to main:
   - 🚀 Combined job for formatting, linting, and testing

These workflows ensure that all code merged into the main branch meets quality standards.

## ⭐ Star Us!

If you find this project useful, please consider giving it a star on GitHub! Your support helps us grow and improve.
