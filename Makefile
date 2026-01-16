# Makefile for RequestRepo v2

# Variables
FRONTEND_DIR := frontend
RUST_DIR := src
HOOKS_DIR := .git/hooks

# Commands
FRONTEND_START_CMD := bun run dev
FORMAT_JS_CMD := cd $(FRONTEND_DIR) && bunx prettier --write --log-level silent

# Install dependencies and git hooks
.PHONY: install
install: install-deps install-hooks

# Install dependencies only
.PHONY: install-deps
install-deps:
	cd $(RUST_DIR) && cargo build --release
	cd $(FRONTEND_DIR) && bun install

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
start-backend:
	if [ ! -f .env ]; then cp .env.example .env; fi
	cd $(RUST_DIR) && cargo run --release

# Start the backend with hot reload (like bun dev)
.PHONY: dev-backend
dev-backend:
	if [ ! -f .env ]; then cp .env.example .env; fi
	cd $(RUST_DIR) && cargo watch -x run

# Start the frontend application
.PHONY: start-frontend
start-frontend:
	cd $(FRONTEND_DIR) && $(FRONTEND_START_CMD)

# Build the backend
.PHONY: build
build:
	cd $(RUST_DIR) && cargo build --release

# Run tests
.PHONY: test
test: test-backend test-frontend

.PHONY: test-backend
test-backend:
	cd $(RUST_DIR) && cargo test

.PHONY: test-frontend
test-frontend:
	cd $(FRONTEND_DIR) && bun run test

# Lint the codebase
.PHONY: lint
lint: lint-js lint-rust

.PHONY: lint-js
lint-js:
	cd $(FRONTEND_DIR) && bun run lint

.PHONY: lint-rust
lint-rust:
	cd $(RUST_DIR) && cargo clippy -- -D warnings

# Format the codebase
.PHONY: format
format: format-js format-rust

.PHONY: format-js
format-js:
	$(FORMAT_JS_CMD) .

.PHONY: format-rust
format-rust:
	cd $(RUST_DIR) && cargo fmt

# Clean build artifacts
.PHONY: clean
clean:
	cd $(RUST_DIR) && cargo clean
	rm -rf $(FRONTEND_DIR)/node_modules $(FRONTEND_DIR)/build

# Docker commands
.PHONY: docker-build
docker-build:
	docker compose build

.PHONY: docker-up
docker-up:
	docker compose up -d

.PHONY: docker-down
docker-down:
	docker compose down
