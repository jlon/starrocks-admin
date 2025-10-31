#!/bin/bash

#
# StarRocks Admin - Optimized Development Environment
# 优化的开发环境启动脚本（排除编译输出和日志目录）
#

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

MODE="${1:-dev}"  # dev: 热重载模式 | start: 后台模式

echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   StarRocks Admin - Optimized Dev     ║${NC}"
echo -e "${BLUE}║   排除编译输出和日志目录优化版本      ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo -e "${YELLOW}Stopping services...${NC}"
    if [ -n "$BACKEND_PID" ]; then
        kill $BACKEND_PID 2>/dev/null || true
        echo -e "${GREEN}✓ Backend stopped${NC}"
    fi
    if [ -n "$FRONTEND_PID" ]; then
        kill $FRONTEND_PID 2>/dev/null || true
        echo -e "${GREEN}✓ Frontend stopped${NC}"
    fi
    exit 0
}

trap cleanup SIGINT SIGTERM

# Check environment
echo -e "${YELLOW}[1/6]${NC} Checking environment..."
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}✗ Rust/Cargo not found. Please install Rust.${NC}"
    exit 1
fi
if ! command -v npm &> /dev/null; then
    echo -e "${RED}✗ Node.js/npm not found. Please install Node.js.${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Environment OK${NC}"
echo ""

# Generate configurations
echo -e "${YELLOW}[2/6]${NC} Generating configurations..."
cd frontend
if npm run config:generate > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Frontend config generated${NC}"
else
    echo -e "${YELLOW}⚠ Using existing frontend config${NC}"
fi
cd "$PROJECT_ROOT"
echo ""

# Pre-compile dependencies (avoid repeated compilation)
echo -e "${YELLOW}[3/6]${NC} Pre-compiling dependencies..."
cd backend
if [ ! -d "target/debug" ] || [ ! -f "target/debug/deps/libstarrocks_admin-*.rlib" ]; then
    echo -e "${YELLOW}First time compilation, this may take a few minutes...${NC}"
    # Pre-compile dependencies in release mode for better performance
    cargo build --release --dependencies-only 2>/dev/null || true
    cargo build
    echo -e "${GREEN}✓ Dependencies compiled${NC}"
else
    echo -e "${GREEN}✓ Dependencies already compiled${NC}"
fi
cd "$PROJECT_ROOT"
echo ""

# Create optimized cargo-watch configuration
echo -e "${YELLOW}[4/6]${NC} Creating optimized file watching configuration..."
cd backend
cat > .cargo-watch.toml << 'EOF'
[watch]
# Only watch source files
paths = ["src", "Cargo.toml"]

# Ignore build artifacts, logs, and temporary files
ignore = [
    "target/**",
    "logs/**", 
    "data/**",
    "conf/**",
    "*.log",
    "*.pid",
    "*.tmp",
    "*.swp",
    "*.swo",
    ".git/**",
    "node_modules/**",
    "dist/**",
    "build/**",
    ".angular/**",
    "coverage/**",
    "e2e/**",
    "*.lock",
    "*.orig",
    "*.rej",
    "**/*.rlib",
    "**/*.rmeta",
    "**/*.d",
    "**/*.pdb",
    "**/*.ilk",
    "**/*.exp",
    "**/*.lib",
    "**/*.a",
    "**/*.so",
    "**/*.dylib",
    "**/*.dll",
    "**/*.exe",
    "**/*.map"
]

# Add delay to prevent rapid recompilation
delay = 1

# Clear screen on rebuild
clear = true

# Run command
run = "run --bin starrocks-admin"
EOF
echo -e "${GREEN}✓ Backend watch configuration created${NC}"
cd "$PROJECT_ROOT"
echo ""

# Start backend with optimized watch
echo -e "${YELLOW}[5/6]${NC} Starting backend with optimized hot reload..."
cd backend

if [ "$MODE" = "dev" ]; then
    cargo watch &
    BACKEND_PID=$!
else
    cargo run --bin starrocks-admin &
    BACKEND_PID=$!
fi

cd "$PROJECT_ROOT"
sleep 5

# Check backend health
if curl -s http://0.0.0.0:8081/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Backend running (PID: $BACKEND_PID)${NC}"
else
    echo -e "${YELLOW}⚠ Backend starting... (PID: $BACKEND_PID)${NC}"
fi
echo ""

# Start frontend with optimized settings
echo -e "${YELLOW}[6/6]${NC} Starting frontend with optimized hot reload..."
cd frontend

# Check if node_modules exists
if [ ! -d "node_modules" ]; then
    echo -e "${YELLOW}Installing frontend dependencies...${NC}"
    npm install
fi

# Start with optimized polling and file watching
npm start &
FRONTEND_PID=$!
sleep 3

echo -e "${YELLOW}Services starting...${NC}"
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo ""
echo -e "${GREEN}Optimized development environment started!${NC}"
echo ""
echo -e "${BLUE}Optimizations applied:${NC}"
echo -e "  • Backend: Pre-compiled dependencies, optimized file watching"
echo -e "  • Frontend: Polling-based change detection (2s interval)"
echo -e "  • Excluded directories: target/, logs/, dist/, build/, .angular/, coverage/"
echo -e "  • Excluded files: *.log, *.pid, *.tmp, *.swp, *.swo, *.orig, *.rej"
echo -e "  • Reduced file watching scope to prevent duplicate triggers"
echo ""
echo -e "${BLUE}Access URLs:${NC}"
echo -e "  Frontend: ${GREEN}http://localhost:4200${NC}"
echo -e "  Backend:  ${GREEN}http://0.0.0.0:8081${NC}"
echo -e "  API Docs: ${GREEN}http://0.0.0.0:8081/api-docs${NC}"
echo ""
if [ "$MODE" = "dev" ]; then
    echo -e "${GREEN}✓ Hot reload enabled - optimized for minimal recompilation${NC}"
    echo -e "${GREEN}✓ Compilation output and log directories excluded${NC}"
fi
echo -e "${YELLOW}Press Ctrl+C to stop all services${NC}"
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo ""

# Wait for processes
wait $FRONTEND_PID

