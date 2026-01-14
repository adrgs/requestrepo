# Contributing to RequestRepo

Thank you for considering contributing to RequestRepo! We appreciate any help that you can provide.

## Getting Started

1. Fork the repository and clone it to your local machine.

2. Make sure you have the required dependencies installed:
   - [Rust](https://rustup.rs/) 1.75+ with cargo
   - [Bun](https://bun.sh/) 1.0+ for the frontend
   - Docker (optional, for deployment testing)

3. Install dependencies using our Makefile:
   ```bash
   make install
   ```

4. Create a new branch for your changes:
   ```bash
   git checkout -b my-feature-branch
   ```

## Development Workflow

We provide a Makefile to help with common development tasks.

### Starting the Services

```bash
# Start the Rust backend (HTTP, DNS, SMTP servers)
make start-backend

# Or with hot reload (requires cargo-watch)
make dev-backend

# Start the frontend development server
make start-frontend
```

### Testing

Run the tests using our Makefile:

```bash
# Run all tests
make test

# Run only backend tests
make test-backend

# Run only frontend tests
make test-frontend
```

### Code Quality

```bash
# Run all linters (clippy + eslint)
make lint

# Run only Rust linting
make lint-rust

# Run only JavaScript/TypeScript linting
make lint-js

# Format all code (cargo fmt + prettier)
make format

# Format only Rust code
make format-rust

# Format only JavaScript/TypeScript code
make format-js
```

## Git Hooks

When you run `make install`, git hooks are automatically set up to ensure code quality:

- **pre-commit**: Runs formatting and linting checks before each commit
- **pre-push**: Runs formatting, linting, and tests before pushing to remote

These hooks help maintain code quality standards across the project.

## Pull Request Process

1. Make your changes in your feature branch
2. Ensure all git hooks pass (format, lint, tests)
3. Commit your changes and push them to your fork
4. Create a Pull Request against the main repository's `main` branch
5. Wait for the GitHub Actions to complete and for a maintainer to review your PR

## Environment Setup

The project uses environment variables for configuration. Copy `.env.example` to `.env` and modify as needed:

```bash
cp .env.example .env
```

Key environment variables:
- `JWT_SECRET`: Secret key for JWT tokens (required)
- `DOMAIN`: Your domain name (required)
- `SERVER_IP`: Your server's public IP address (required)
- `TLS_ENABLED`: Enable HTTPS with auto-TLS (optional)
- `ADMIN_TOKEN`: Require password for session creation (optional)

See `.env.example` for all available options.

## Project Structure

```
requestrepo/
├── src/              # Rust backend
│   └── src/
│       ├── cache/    # In-memory LRU cache
│       ├── certs/    # TLS/ACME management
│       ├── dns/      # DNS server
│       ├── http/     # HTTP server + API
│       ├── smtp/     # SMTP server
│       └── tests/    # Backend tests
├── frontend/         # React frontend
│   └── src/
│       ├── components/
│       ├── pages/
│       ├── stores/   # Zustand state
│       └── hooks/
└── Makefile          # Development commands
```

## Contact

If you have any questions or doubts, feel free to:
- Open an issue on GitHub
- Email us at [contact@requestrepo.com](mailto:contact@requestrepo.com)
