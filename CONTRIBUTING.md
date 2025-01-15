# Contributing to requestrepo

Thank you for considering contributing to requestrepo! We appreciate any help that you can provide.

## Getting Started

1. Fork the repository and clone it to your local machine.

2. Make sure you have the required dependencies installed:
   - [Python 3.11+](https://www.python.org/)
   - [Poetry](https://python-poetry.org/) for Python dependency management
   - [Node.js](https://nodejs.org/) and [npm](https://www.npmjs.com/) for the frontend
   - [Redis](https://redis.io/) for data storage

3. Install dependencies:
   ```bash
   # Install Python dependencies using Poetry
   poetry install

   # Install frontend dependencies
   cd frontend
   npm install
   ```

4. Create a new branch for your changes:
   ```bash
   git checkout -b my-feature-branch
   ```

## Development

We provide a Makefile to help with common development tasks:

### Starting the Services

```bash
# Start Redis for development
make start-redis

# Start the backend server (will also start Redis if not running)
make start-backend

# Start the frontend development server
make start-frontend
```

### Code Quality

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

### Makefile Commands Reference

- `start-redis`: Starts a Redis container for development
- `start-backend`: Starts the backend server using Poetry and uvicorn
- `start-frontend`: Starts the frontend development server using npm
- `lint`: Runs all code linters
- `lint-js`: Runs ESLint on frontend code
- `lint-python`: Runs Ruff on Python code
- `format`: Formats all code
- `format-js`: Formats JavaScript/TypeScript using Prettier
- `format-python`: Formats Python code using Ruff

## Testing

Run the tests using Poetry:

```bash
poetry run pytest
```

## Pull Request Process

1. Make your changes in your feature branch
2. Run the linters and formatters:
   ```bash
   make lint
   make format
   ```
3. Run the tests to ensure everything works:
   ```bash
   poetry run pytest
   ```
4. Commit your changes and push them to your fork
5. Create a Pull Request against the main repository's `main` branch

## Environment Setup

The project uses environment variables for configuration. Copy `.env.example` to `.env` and modify as needed:

```bash
cp .env.example .env
```

Key environment variables:
- `JWT_SECRET`: Secret key for JWT tokens
- `DOMAIN`: Your domain name
- `SERVER_IP`: Your server's IP address
- `REDIS_TTL_DAYS`: How long to keep data in Redis (default: 7 days)

## Contact

If you have any questions or doubts, feel free to contact the maintainers by opening an issue.