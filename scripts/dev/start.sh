#!/bin/bash

#
# StarRocks Admin - Development Environment (One-Click Start)
# 开发环境一键启动（同时启动前后端，支持热加载）
#

set -e

# 全局变量用于跟踪前端启动状态
FRONTEND_PID=""

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
echo -e "${BLUE}║   StarRocks Admin - Dev Environment   ║${NC}"
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
echo -e "${YELLOW}[1/4]${NC} Checking environment..."
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

# 加载 Rust 环境（如果存在）
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# 加载公共函数
source "$PROJECT_ROOT/scripts/dev/common.sh"

# Start backend (确保在 WSL 中运行)
echo -e "${YELLOW}[2/4]${NC} Starting backend..."
echo -e "${BLUE}验证后端运行环境...${NC}"

# 验证当前在 WSL 环境中
print_wsl_status || echo -e "${YELLOW}继续尝试启动...${NC}"

# 验证 Rust 工具链
if ! verify_wsl_tool "cargo" "Cargo"; then
    echo -e "${YELLOW}请确保 Rust 工具链已安装在 WSL 中${NC}"
    exit 1
fi

if [ "$MODE" = "dev" ]; then
    echo -e "${YELLOW}启动后端 (优化热加载模式)...${NC}"
    bash scripts/dev/start_backend_dev_optimized.sh &
    BACKEND_PID=$!
    sleep 5
else
    echo -e "${YELLOW}启动后端 (标准模式)...${NC}"
bash scripts/dev/start_backend.sh &
BACKEND_PID=$!
sleep 3
fi

# Check backend health
if curl -s http://0.0.0.0:8081/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Backend running (PID: $BACKEND_PID)${NC}"
else
    echo -e "${YELLOW}⚠ Backend starting... (PID: $BACKEND_PID)${NC}"
fi
echo ""

# Start frontend (确保在 WSL 中运行)
echo -e "${YELLOW}[3/4]${NC} Starting frontend (in WSL)..."
cd frontend

# 验证 Node.js 在 WSL 中可用
if ! command -v node > /dev/null 2>&1; then
    echo -e "${RED}✗ Node.js 未在 WSL 中安装${NC}"
    echo ""
    
    # 检查是否在交互式终端中
    if [ -t 0 ]; then
        echo -e "${YELLOW}是否要现在安装 Node.js? (y/N)${NC}"
        read -p "> " -n 1 -r
        echo ""
        
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${BLUE}正在安装 Node.js...${NC}"
            bash "$PROJECT_ROOT/scripts/dev/install_nodejs.sh"
            # 重新加载 PATH
            export PATH="$PATH"
        else
            echo -e "${YELLOW}已跳过安装${NC}"
            echo ""
            echo -e "您可以稍后手动安装 Node.js："
            echo -e "  bash scripts/dev/install_nodejs.sh"
            echo ""
            echo -e "或使用命令："
            echo -e "  curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -"
            echo -e "  sudo apt-get install -y nodejs"
            exit 1
        fi
    else
        # 非交互式模式，提示用户手动安装
        echo -e "${YELLOW}非交互式模式，无法自动安装${NC}"
        echo ""
        echo -e "请手动安装 Node.js："
        echo -e "  bash scripts/dev/install_nodejs.sh"
        echo ""
        echo -e "或使用命令："
        echo -e "  curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -"
        echo -e "  sudo apt-get install -y nodejs"
        exit 1
    fi
fi

# 检查 Node.js 版本
local node_version=$(node --version 2>/dev/null || echo "unknown")
echo -e "${GREEN}✓ Node.js 版本: $node_version${NC}"

# 启动前端服务（即使依赖安装失败也尝试启动）
FRONTEND_STARTED=false

# 验证当前在 WSL 环境中
echo -e "${YELLOW}验证运行环境...${NC}"
print_wsl_status || echo -e "${YELLOW}继续尝试启动...${NC}"

# Check if node_modules exists
if [ ! -d "node_modules" ]; then
    echo -e "${YELLOW}安装前端依赖...${NC}"
    set +e  # 临时禁用错误退出
    npm install 2>&1
    local npm_install_result=$?
    set -e  # 重新启用错误退出
    
    if [ $npm_install_result -ne 0 ]; then
        echo -e "${RED}✗ 前端依赖安装失败，但将继续尝试启动${NC}"
        echo -e "${YELLOW}如果启动失败，请手动运行: cd frontend && npm install${NC}"
    fi
fi

# 确保使用 WSL 的 Node.js 启动
if ! verify_wsl_tool "node" "Node.js"; then
    echo -e "${YELLOW}请确保在 WSL 中安装并使用 WSL 版本的 Node.js：${NC}"
    echo -e "  bash scripts/dev/install_nodejs.sh"
    echo ""
    
    # 只在交互模式下询问
    if [ -t 0 ]; then
        read -p "是否继续使用当前 Node.js? (y/N): " -n 1 -r
        echo ""
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${YELLOW}已取消启动${NC}"
            FRONTEND_PID=""
            FRONTEND_STARTED=false
            cd "$PROJECT_ROOT"
            exit 1
        fi
    else
        echo -e "${RED}✗ 非交互模式：拒绝使用 Windows 版本的 Node.js${NC}"
        echo -e "${YELLOW}请在 WSL 中安装 Node.js 后重试${NC}"
        FRONTEND_PID=""
        FRONTEND_STARTED=false
        cd "$PROJECT_ROOT"
        exit 1
    fi
fi

# 创建日志目录
mkdir -p "$PROJECT_ROOT/frontend"
LOG_FILE="$PROJECT_ROOT/frontend/frontend.log"

echo -e "${YELLOW}启动前端服务...${NC}"
echo -e "${BLUE}日志文件: $LOG_FILE${NC}"

# 启动前端服务（后台运行，输出到日志文件）
cd "$PROJECT_ROOT/frontend"

echo -e "${YELLOW}正在启动前端服务...${NC}"

# 先停止可能存在的旧进程
set +e
if command -v lsof > /dev/null 2>&1; then
    lsof -ti :4200 | xargs kill -9 2>/dev/null || true
fi
set -e

# 启动前端服务（使用 nohup 和后台运行）
# package.json 中已经包含了 --host 0.0.0.0 --disable-host-check --poll 2000
nohup npm start > "$LOG_FILE" 2>&1 &
FRONTEND_PID=$!

echo -e "${BLUE}前端进程 PID: $FRONTEND_PID${NC}"
echo -e "${BLUE}等待前端服务启动...${NC}"

# 等待更长时间让前端完全启动
sleep 8

# 检查前端是否成功启动（多种方式检查）
set +e  # 临时禁用错误退出
local frontend_running=false

# 方式1: 检查进程是否还在运行
if ps -p "$FRONTEND_PID" > /dev/null 2>&1; then
    frontend_running=true
    echo -e "${GREEN}✓ 前端进程正在运行 (PID: $FRONTEND_PID)${NC}"
else
    echo -e "${YELLOW}⚠ 前端进程已退出 (PID: $FRONTEND_PID)${NC}"
fi

# 方式2: 检查端口是否在监听
if [ "$frontend_running" = false ]; then
    if command -v lsof > /dev/null 2>&1; then
        local port_pid=$(lsof -ti :4200 2>/dev/null | head -1)
        if [ -n "$port_pid" ]; then
            FRONTEND_PID="$port_pid"
            frontend_running=true
            echo -e "${GREEN}✓ 检测到前端服务监听端口 4200 (PID: $port_pid)${NC}"
        fi
    fi
fi

# 方式3: 检查日志中是否有成功启动的标记
if [ "$frontend_running" = false ] && [ -f "$LOG_FILE" ]; then
    if grep -q "Angular Live Development Server is listening" "$LOG_FILE" 2>/dev/null; then
        # 尝试找到实际的进程 PID
        if command -v pgrep > /dev/null 2>&1; then
            local ng_pid=$(pgrep -f "ng serve" | head -1)
            if [ -n "$ng_pid" ]; then
                FRONTEND_PID="$ng_pid"
                frontend_running=true
                echo -e "${GREEN}✓ 从日志检测到前端服务已启动 (PID: $ng_pid)${NC}"
            fi
        fi
    fi
fi

set -e  # 重新启用错误退出

if [ "$frontend_running" = true ]; then
    echo -e "${GREEN}✓ 前端服务启动成功 (PID: $FRONTEND_PID)${NC}"
    FRONTEND_STARTED=true
else
    echo -e "${RED}✗ 前端服务启动失败${NC}"
    echo -e "${YELLOW}请查看日志获取详细信息:${NC}"
    echo -e "  tail -n 50 $LOG_FILE"
    echo ""
    if [ -f "$LOG_FILE" ] && [ -s "$LOG_FILE" ]; then
        echo -e "${YELLOW}最近的日志 (最后20行):${NC}"
        tail -n 20 "$LOG_FILE"
    else
        echo -e "${YELLOW}日志文件为空或不存在${NC}"
    fi
    FRONTEND_PID=""
fi

cd "$PROJECT_ROOT"

echo -e "${YELLOW}[4/4]${NC} Services starting..."
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo -e ""
echo -e "${GREEN}Development environment started!${NC}"
echo -e ""
echo -e "${BLUE}Access URLs:${NC}"
if [ "$FRONTEND_STARTED" = true ]; then
    echo -e "  Frontend: ${GREEN}http://localhost:4200${NC} ✓"
else
    echo -e "  Frontend: ${RED}未启动${NC} ✗"
fi
echo -e "  Backend:  ${GREEN}http://0.0.0.0:8081${NC} ✓"
echo -e "  API Docs: ${GREEN}http://0.0.0.0:8081/api-docs${NC}"
echo -e ""
if [ "$FRONTEND_STARTED" = false ]; then
    echo -e "${YELLOW}⚠ 前端服务未启动，后端服务继续运行${NC}"
    echo -e "${YELLOW}请检查前端日志或手动启动前端${NC}"
fi
if [ "$MODE" = "dev" ]; then
    echo -e "${GREEN}✓ Hot reload enabled - code changes auto-reload${NC}"
fi
echo -e "${YELLOW}Press Ctrl+C to stop all services${NC}"
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo ""

# Wait for processes
if [ -n "$FRONTEND_PID" ] && [ "$FRONTEND_PID" != "" ]; then
    echo -e "${GREEN}前后端服务都在运行，等待中... (按 Ctrl+C 停止)${NC}"
wait $FRONTEND_PID
else
    echo -e "${YELLOW}前端服务未启动，只有后端在运行 (按 Ctrl+C 停止)${NC}"
    # 等待后端进程或用户中断
    wait $BACKEND_PID 2>/dev/null || true
fi
