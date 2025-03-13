# Makefile

# Variables
FRONTEND_DIR := frontend
BACKEND_DIR := backend
PYTHON_DIR := backend  # Assuming your Python code is in the backend directory
LINT_PATHS := $(FRONTEND_DIR) $(BACKEND_DIR)
HOOKS_DIR := .git/hooks

# Commands
FRONTEND_START_CMD := npm run dev
BACKEND_START_CMD := poetry run uvicorn app:app --port 21337 --no-server-header --reload
FRONTEND_LINT_CMD := npm run lint
PYTHON_LINT_CMD := poetry run ruff check
FORMAT_PYTHON := poetry run ruff format
FORMAT_JS_CMD := prettier --write --log-level silent
REDIS_CONTAINER_NAME := redis-requestrepo-dev
REDIS_PORT := 6379

# Install dependencies and git hooks
.PHONY: install
install: install-deps install-hooks

# Install dependencies only
.PHONY: install-deps
install-deps:
	poetry install
	cd $(FRONTEND_DIR) && npm install --legacy-peer-deps

# Install git hooks
.PHONY: install-hooks
install-hooks:
	@mkdir -p $(HOOKS_DIR)
	@echo "#!/bin/sh\nmake pre-commit" > $(HOOKS_DIR)/pre-commit
	@echo "#!/bin/sh\nmake pre-push" > $(HOOKS_DIR)/pre-push
	@chmod +x $(HOOKS_DIR)/pre-commit $(HOOKS_DIR)/pre-push
	@echo "Git hooks installed successfully"

# Pre-commit hook: format and lint
.PHONY: pre-commit
pre-commit: format lint
	@echo "Pre-commit checks passed!"

# Pre-push hook: format, lint, and test
.PHONY: pre-push
pre-push: format lint test
	@echo "Pre-push checks passed!"

# Start the backend server
.PHONY: start-backend
start-backend: start-redis
	if [ ! -f .env ]; then cp .env.example .env; fi
	cd $(BACKEND_DIR) && $(BACKEND_START_CMD)

# Start the Redis container
.PHONY: start-redis
start-redis:
	@if [ `docker ps -q -f name=$(REDIS_CONTAINER_NAME)` ]; then \
		echo "$(REDIS_CONTAINER_NAME) is already running"; \
	else \
		if [ `docker ps -aq -f name=$(REDIS_CONTAINER_NAME)` ]; then \
			docker start $(REDIS_CONTAINER_NAME); \
		else \
			docker run -d --name $(REDIS_CONTAINER_NAME) -p $(REDIS_PORT):6379 redis; \
		fi \
	fi

# Stop the Redis container
.PHONY: stop-redis
stop-redis:
	@if [ `docker ps -q -f name=$(REDIS_CONTAINER_NAME)` ]; then \
		docker stop $(REDIS_CONTAINER_NAME); \
	else \
		echo "$(REDIS_CONTAINER_NAME) is not running"; \
	fi

# Start the frontend application
.PHONY: start-frontend
start-frontend:
	cd $(FRONTEND_DIR) && $(FRONTEND_START_CMD)

# Run tests
.PHONY: test
test: test-backend test-dns

.PHONY: test-backend
test-backend:
	poetry run pytest backend/tests

.PHONY: test-dns
test-dns:
	poetry run pytest dns/tests

# Lint the codebase
.PHONY: lint
lint: lint-js lint-python

.PHONY: lint-js
lint-js:
	cd $(FRONTEND_DIR) && $(FRONTEND_LINT_CMD)

.PHONY: lint-python
lint-python:
	$(PYTHON_LINT_CMD) $(PYTHON_DIR)

# Format the codebase
.PHONY: format
format: format-js format-python

.PHONY: format-js
format-js:
	$(FORMAT_JS_CMD) $(FRONTEND_DIR)

.PHONY: format-python
format-python:
	$(FORMAT_PYTHON) $(PYTHON_DIR)