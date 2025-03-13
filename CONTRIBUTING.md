# 🤝 Contributing to requestrepo

Thank you for considering contributing to requestrepo! We appreciate any help that you can provide.

## 🚀 Getting Started

1. Fork the repository and clone it to your local machine.

2. Make sure you have the required dependencies installed:
   - 🐍 [Python 3.11+](https://www.python.org/)
   - 📜 [Poetry](https://python-poetry.org/) for Python dependency management
   - 📦 [Node.js](https://nodejs.org/) and [npm](https://www.npmjs.com/) for the frontend
   - 💾 [Redis](https://redis.io/) for data storage

3. Install dependencies using our Makefile:
   ```bash
   # Install all dependencies and git hooks
   make install
   ```

4. Create a new branch for your changes:
   ```bash
   git checkout -b my-feature-branch
   ```

## 💻 Development Workflow

We provide a Makefile to help with common development tasks:

### 🚀 Starting the Services

```bash
# Start Redis for development
make start-redis

# Start the backend server (will also start Redis if not running)
make start-backend

# Start the frontend development server
make start-frontend
```

### 🔌 Starting the DNS Server

The DNS server needs to be started manually:

```bash
# Start the DNS server
cd dns; python ns.py
```

### 🧪 Testing

Run the tests using our Makefile:

```bash
# Run all tests
make test

# Run only backend tests
make test-backend

# Run only DNS tests
make test-dns
```

### ✨ Code Quality

```bash
# Run all linters
make lint

# Run only JavaScript/TypeScript linting
make lint-js

# Run only Python linting
make lint-python

# Format all code
make format

# Format only JavaScript/TypeScript code
make format-js

# Format only Python code
make format-python
```

## 🔗 Git Hooks

When you run `make install`, git hooks are automatically set up to ensure code quality:

- **pre-commit**: Runs formatting and linting checks before each commit
- **pre-push**: Runs formatting, linting, and tests before pushing to remote

These hooks help maintain code quality standards across the project.

## 📝 Pull Request Process

1. Make your changes in your feature branch
2. Ensure all git hooks pass (format, lint, tests)
3. Commit your changes and push them to your fork
4. Create a Pull Request against the main repository's `main` branch
5. Wait for the GitHub Actions to complete and for a maintainer to review your PR

## ⚙️ Environment Setup

The project uses environment variables for configuration. Copy `.env.example` to `.env` and modify as needed:

```bash
cp .env.example .env
```

Key environment variables:
- 🔑 `JWT_SECRET`: Secret key for JWT tokens
- 🌐 `DOMAIN`: Your domain name
- 🖥️ `SERVER_IP`: Your server's IP address
- ⏱️ `REDIS_TTL_DAYS`: How long to keep data in Redis (default: 7 days)

## 📱 Contact

If you have any questions or doubts, feel free to:
- 🐛 Open an issue on GitHub
- 💬 Join our [Discord](https://discord.gg/requestrepo)
- 📧 Email us at [contact@requestrepo.com](mailto:contact@requestrepo.com)