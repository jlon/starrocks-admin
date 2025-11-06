.PHONY: help build docker-build docker-up docker-down clean

# Project paths
PROJECT_ROOT := $(shell pwd)
BACKEND_DIR := $(PROJECT_ROOT)/backend
FRONTEND_DIR := $(PROJECT_ROOT)/frontend
BUILD_DIR := $(PROJECT_ROOT)/build
DIST_DIR := $(BUILD_DIR)/dist

# Default target - show help
help:
	@echo "StarRocks Admin - Build Commands:"
	@echo ""
	@echo "Build:"
	@echo "  make build        - Build backend and frontend, then create distribution package"
	@echo "  make docker-build - Build Docker image"
	@echo "  make docker-up    - Start Docker container"
	@echo "  make docker-down  - Stop Docker container"
	@echo "  make clean        - Clean build artifacts"
	@echo ""

# Build both backend and frontend, then create distribution package
build:
	@echo "Running clippy checks on backend..."
	@cd $(BACKEND_DIR) && cargo clippy --release --all-targets -- --deny warnings --allow clippy::uninlined-format-args
	@echo "✓ Clippy checks passed!"
	@echo ""
	@echo "Building StarRocks Admin..."
	@bash build/build-backend.sh
	@bash build/build-frontend.sh
	@echo "Build complete! Output: $(DIST_DIR)"
	@echo "Creating distribution package..."
	@TIMESTAMP=$$(date +"%Y%m%d"); \
	PACKAGE_NAME="starrocks-admin-$$TIMESTAMP.tar.gz"; \
	PACKAGE_PATH="$(DIST_DIR)/$$PACKAGE_NAME"; \
	echo "Package name: $$PACKAGE_NAME"; \
	cd $(DIST_DIR) && tar -czf "$$PACKAGE_NAME" --transform 's,^,starrocks-admin/,' *; \
	echo "Package created: $$PACKAGE_PATH"; \
	echo "To extract: tar -xzf $$PACKAGE_NAME"

# Build Docker image
docker-build:
	@echo "Building Docker image..."
	@docker build -f deploy/docker/Dockerfile -t starrocks-admin .

# Start Docker container
docker-up:
	@echo "Starting Docker container..."
	@cd deploy/docker && docker compose up -d

# Stop Docker container
docker-down:
	@echo "Stopping Docker container..."
	@cd deploy/docker && docker compose down

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@rm -rf $(BUILD_DIR)
	@cd $(BACKEND_DIR) && cargo clean
	@cd $(FRONTEND_DIR) && rm -rf dist node_modules/.cache
	@echo "Clean complete!"