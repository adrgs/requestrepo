# 🚀 requestrepo.com

<img src="https://rasp.go.ro/reqlogo.svg" width="420">

🔍 Analyze HTTP and DNS requests and create custom DNS records for your subdomain.

## 🏁 Getting Started

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