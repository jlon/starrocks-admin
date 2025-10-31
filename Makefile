.PHONY: help build build-force build-clean docker-build docker-up docker-down clean
.PHONY: dev dev-start dev-stop dev-restart dev-status dev-logs

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
	@echo "  make build        - 增量构建（仅构建有修改的组件）"
	@echo "  make build-force  - 强制构建所有组件"
	@echo "  make build-clean  - 清理缓存后构建"
	@echo ""
	@echo "Development:"
	@echo "  make dev          - 启动开发环境（前后端分离终端窗口，方便查看日志）"
	@echo "  make dev-start    - 启动开发服务器（后台）"
	@echo "  make dev-stop     - 停止开发服务器"
	@echo "  make dev-restart  - 重启开发服务器"
	@echo "  make dev-status   - 查看服务状态"
	@echo "  make dev-logs     - 查看实时日志"
	@echo ""
	@echo "Docker:"
	@echo "  make docker-build - Build Docker image"
	@echo "  make docker-up    - Start Docker container"
	@echo "  make docker-down  - Stop Docker container"
	@echo ""
	@echo "Clean:"
	@echo "  make clean        - Clean build artifacts"
	@echo ""

# 增量构建（默认）
build:
	@bash build/build-optimized.sh

# 强制构建所有组件
build-force:
	@bash build/build-optimized.sh --force

# 清理缓存后构建
build-clean:
	@bash build/build-optimized.sh --clean

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

# Clean build artifacts (完全清理，包括 build 目录)
# 复用 build-clean 的逻辑，然后额外清理 build 目录
clean: build-clean
	@echo "Cleaning build directory..."
	@rm -rf $(BUILD_DIR)
	@echo "Clean complete!"

# ==================== Development Commands ====================

# 开发环境 - 分离终端窗口（前后端在不同窗口，方便查看日志）
dev:
	@echo "Starting development environment in separate terminal windows..."
	@bash scripts/dev/start_separate_terminals.sh

# dev-separate 是 dev 的别名（保持向后兼容）
dev-separate: dev

# 启动开发服务器（分离终端窗口，后台模式）
dev-start:
	@bash scripts/dev/start_separate_terminals.sh start

# 停止开发服务器
dev-stop:
	@bash scripts/dev/stop.sh

# 重启开发服务器（复用 dev-stop 和 dev-start）
dev-restart:
	@$(MAKE) dev-stop
	@sleep 2
	@$(MAKE) dev-start

# 查看服务状态（智能检测，支持所有启动方式）
dev-status:
	@bash scripts/dev/check_status.sh all || true

# 查看实时日志
dev-logs:
	@echo "Showing real-time logs (Ctrl+C to exit)..."
	@bash scripts/dev/logs.sh