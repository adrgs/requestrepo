# Makefile

# Variables
FRONTEND_DIR := frontend
BACKEND_DIR := backend
PYTHON_DIR := backend  # Assuming your Python code is in the backend directory
LINT_PATHS := $(FRONTEND_DIR) $(BACKEND_DIR)

# Commands
FRONTEND_START_CMD := npm run dev
BACKEND_START_CMD := uvicorn app:app --port 21337 --no-server-header --reload
FRONTEND_LINT_CMD := npm run lint
PYTHON_LINT_CMD := ruff check
FORMAT_PYTHON := ruff format
FORMAT_JS_CMD := prettier --write --log-level silent

# Default target
.PHONY: run
run: start-frontend start-backend

# Start the backend server
.PHONY: start-backend
start-backend:
	cd $(BACKEND_DIR) && $(BACKEND_START_CMD)

# Start the frontend application
.PHONY: start-frontend
start-frontend:
	cd $(FRONTEND_DIR) && $(FRONTEND_START_CMD)

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