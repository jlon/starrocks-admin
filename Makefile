.PHONY: help build test format clippy run clean ci install docker-build docker-up docker-down

# ============================================================================
# NOTE: This Makefile is a compatibility layer for the new cargo xtask system
# 
# New users should use: cargo xtask <command>
# Old users can still use: make <command>
#
# See xtask/README.md for detailed documentation
# ============================================================================

# Default target - show help
help:
	@echo "StarRocks Admin - Build Commands (Makefile Compatibility Layer)"
	@echo ""
	@echo "⚠️  NOTICE: We now use 'cargo xtask' as the primary build system"
	@echo "   This Makefile is kept for backward compatibility"
	@echo "   New command: cargo xtask <command>"
	@echo ""
	@echo "Build Commands:"
	@echo "  make build              - Build backend and frontend (calls: cargo xtask build --release)"
	@echo "  make test               - Run all tests (calls: cargo xtask test)"
	@echo "  make format             - Format code (calls: cargo xtask format)"
	@echo "  make clippy             - Run clippy checks (calls: cargo xtask clippy)"
	@echo "  make run                - Build and run (calls: cargo xtask run)"
	@echo "  make clean              - Clean build artifacts (calls: cargo xtask clean)"
	@echo "  make ci                 - Run CI pipeline (calls: cargo xtask ci)"
	@echo "  make dist               - Create distribution package (calls: cargo xtask dist)"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-build       - Build Docker image"
	@echo "  make docker-up          - Start Docker container"
	@echo "  make docker-down        - Stop Docker container"
	@echo ""
	@echo "For more options, run: cargo xtask --help"
	@echo ""

# Build (delegates to cargo xtask)
build:
	@echo "🔄 Delegating to: cargo xtask build --release"
	@cargo xtask build --release

# Test (delegates to cargo xtask)
test:
	@echo "🔄 Delegating to: cargo xtask test"
	@cargo xtask test

# Format (delegates to cargo xtask)
format:
	@echo "🔄 Delegating to: cargo xtask format"
	@cargo xtask format

# Clippy (delegates to cargo xtask)
clippy:
	@echo "🔄 Delegating to: cargo xtask clippy"
	@cargo xtask clippy

# Run (delegates to cargo xtask)
run:
	@echo "🔄 Delegating to: cargo xtask run"
	@cargo xtask run

# Clean (delegates to cargo xtask)
clean:
	@echo "🔄 Delegating to: cargo xtask clean"
	@cargo xtask clean

# CI pipeline (delegates to cargo xtask)
ci:
	@echo "🔄 Delegating to: cargo xtask ci"
	@cargo xtask ci

# Create distribution package (delegates to cargo xtask)
dist:
	@echo "🔄 Delegating to: cargo xtask dist"
	@cargo xtask dist

# Build Docker image
docker-build:
	@echo "Building Docker image..."
	@docker build -f deploy/docker/Dockerfile -t starrocks-admin:latest .

# Start Docker container without rebuild (use existing image)
docker-up:
	@echo "Starting Docker container (using existing image)..."
	@cd deploy/docker && docker compose up -d

# Stop Docker container
docker-down:
	@echo "Stopping Docker container..."
	@cd deploy/docker && docker compose down