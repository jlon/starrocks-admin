#!/bin/bash

# StarRocks Admin - Development Environment (Separate Terminals)
# 开发环境启动（前后端在不同终端窗口中运行，方便查看日志）

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
echo -e "${BLUE}║   StarRocks Admin - Dev Environment   ║${NC}"
echo -e "${BLUE}║      (Separate Terminal Windows)      ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# 如果是 dev 模式，执行增量构建优化
if [ "$MODE" = "dev" ]; then
    echo -e "${YELLOW}准备开发环境（增量构建优化）...${NC}"
    if [ -f "$PROJECT_ROOT/scripts/dev/incremental_build.sh" ]; then
        bash "$PROJECT_ROOT/scripts/dev/incremental_build.sh"
        echo ""
    fi
fi

# 检测可用的终端
detect_terminal() {
    # Windows Terminal (WSL)
    if command -v wt.exe > /dev/null 2>&1; then
        echo "wt"
        return 0
    fi
    
    # Windows Terminal (直接调用)
    if [ -n "$WT_SESSION" ]; then
        echo "wt"
        return 0
    fi
    
    # gnome-terminal (Linux GUI)
    if command -v gnome-terminal > /dev/null 2>&1 && [ -n "$DISPLAY" ]; then
        echo "gnome-terminal"
        return 0
    fi
    
    # xterm (Linux)
    if command -v xterm > /dev/null 2>&1 && [ -n "$DISPLAY" ]; then
        echo "xterm"
        return 0
    fi
    
    # tmux (终端复用器)
    if command -v tmux > /dev/null 2>&1; then
        echo "tmux"
        return 0
    fi
    
    # screen (终端复用器)
    if command -v screen > /dev/null 2>&1; then
        echo "screen"
        return 0
    fi
    
    echo "none"
    return 1
}

# 在新终端中启动后端
start_backend_in_terminal() {
    local terminal_type=$1
    local backend_script=""
    
    if [ "$MODE" = "dev" ]; then
        backend_script="scripts/dev/start_backend_dev_optimized.sh"
    else
        backend_script="scripts/dev/start_backend.sh"
    fi
    
    case "$terminal_type" in
        wt)
            echo -e "${YELLOW}在 Windows Terminal (WSL) 中启动后端...${NC}"
            # 确保在 WSL 环境中启动
            echo -e "${BLUE}使用 WSL 默认发行版${NC}"
            
            # 创建一个临时启动脚本，避免复杂的引号嵌套
            # 在脚本中添加环境验证
            TEMP_START_SCRIPT="/tmp/start_backend_$$.sh"
            cat > "$TEMP_START_SCRIPT" << EOF
#!/bin/bash
set -e

# 加载 Rust 环境（如果存在）
if [ -f "\$HOME/.cargo/env" ]; then
    source "\$HOME/.cargo/env"
fi

# 验证 WSL 环境
echo "=== 验证 WSL 环境 ==="
uname -a
echo ""
echo "=== Rust/Cargo 路径 ==="
which cargo || echo "Cargo 未找到"
cargo --version || echo "无法获取 Cargo 版本"
echo ""
echo "=== 启动后端服务 (WSL) ==="

cd "$PROJECT_ROOT"
if [ "$MODE" = "start" ]; then
    bash "$backend_script" start
else
    bash "$backend_script"
fi
EOF
            chmod +x "$TEMP_START_SCRIPT"
            
            # 使用临时脚本启动（不指定 -d 参数，使用默认发行版）
            # 明确使用 wsl 命令确保在 WSL 中运行
            wt.exe -w 0 new-tab --title "StarRocks Admin - Backend (WSL)" wsl bash "$TEMP_START_SCRIPT"
            echo -e "${GREEN}✓ 后端启动命令已执行${NC}"
            ;;
        gnome-terminal)
            echo -e "${YELLOW}在 Gnome Terminal 中启动后端...${NC}"
            if [ "$MODE" = "start" ]; then
                gnome-terminal --title="StarRocks Admin - Backend" -- bash -c "cd \"$PROJECT_ROOT\" && bash $backend_script start; exec bash"
            else
                gnome-terminal --title="StarRocks Admin - Backend" -- bash -c "cd \"$PROJECT_ROOT\" && bash $backend_script; exec bash"
            fi
            ;;
        xterm)
            echo -e "${YELLOW}在 XTerm 中启动后端...${NC}"
            if [ "$MODE" = "start" ]; then
                xterm -T "StarRocks Admin - Backend" -e bash -c "cd \"$PROJECT_ROOT\" && bash $backend_script start; exec bash" &
            else
                xterm -T "StarRocks Admin - Backend" -e bash -c "cd \"$PROJECT_ROOT\" && bash $backend_script; exec bash" &
            fi
            ;;
        tmux)
            echo -e "${YELLOW}在 Tmux 会话中启动后端...${NC}"
            if [ "$MODE" = "start" ]; then
                tmux new-session -d -s starrocks-backend "cd \"$PROJECT_ROOT\" && bash $backend_script start"
            else
                tmux new-session -d -s starrocks-backend "cd \"$PROJECT_ROOT\" && bash $backend_script"
            fi
            echo -e "${GREEN}后端在 tmux 会话 'starrocks-backend' 中运行${NC}"
            echo -e "${YELLOW}使用 'tmux attach -t starrocks-backend' 查看${NC}"
            ;;
        screen)
            echo -e "${YELLOW}在 Screen 会话中启动后端...${NC}"
            if [ "$MODE" = "start" ]; then
                screen -dmS starrocks-backend bash -c "cd \"$PROJECT_ROOT\" && bash $backend_script start"
            else
                screen -dmS starrocks-backend bash -c "cd \"$PROJECT_ROOT\" && bash $backend_script"
            fi
            echo -e "${GREEN}后端在 screen 会话 'starrocks-backend' 中运行${NC}"
            echo -e "${YELLOW}使用 'screen -r starrocks-backend' 查看${NC}"
            ;;
        *)
            echo -e "${RED}无法在新终端中启动后端${NC}"
            return 1
            ;;
    esac
}

# 在新终端中启动前端
start_frontend_in_terminal() {
    local terminal_type=$1
    
    case "$terminal_type" in
        wt)
            echo -e "${YELLOW}在 Windows Terminal (WSL) 中启动前端...${NC}"
            # 确保在 WSL 环境中启动，而不是 Windows PowerShell 或 CMD
            # 简化：不指定 -d 参数，让 WSL 使用默认发行版
            echo -e "${BLUE}使用 WSL 默认发行版${NC}"
            
            FRONTEND_DIR="$PROJECT_ROOT/frontend"
            # 创建一个临时启动脚本，避免复杂的引号嵌套
            TEMP_START_SCRIPT="/tmp/start_frontend_$$.sh"
            cat > "$TEMP_START_SCRIPT" << EOF
#!/bin/bash
cd "$FRONTEND_DIR"
echo '=== 验证 WSL 环境 ==='
uname -a
echo ''
echo '=== Node.js 路径 ==='
which node
node --version
echo ''
echo '=== 启动前端服务 ==='
npm start
EOF
            chmod +x "$TEMP_START_SCRIPT"
            # package.json 中已经包含了 --host 0.0.0.0 --disable-host-check --poll 2000
            # 使用临时脚本启动（不指定 -d 参数，使用默认发行版）
            wt.exe -w 0 new-tab --title "StarRocks Admin - Frontend (WSL)" wsl bash "$TEMP_START_SCRIPT"
            echo -e "${GREEN}✓ 前端启动命令已执行${NC}"
            ;;
        gnome-terminal)
            echo -e "${YELLOW}在 Gnome Terminal (WSL) 中启动前端...${NC}"
            # package.json 中已经包含了 --host 0.0.0.0 --disable-host-check --poll 2000
            gnome-terminal --title="StarRocks Admin - Frontend (WSL)" -- bash -c "cd \"$PROJECT_ROOT/frontend\" && echo '=== 验证 WSL 环境 ===' && uname -a && echo '' && echo '=== Node.js 路径 ===' && which node && node --version && echo '' && echo '=== 启动前端服务 ===' && npm start; exec bash"
            ;;
        xterm)
            echo -e "${YELLOW}在 XTerm (WSL) 中启动前端...${NC}"
            # package.json 中已经包含了 --host 0.0.0.0 --disable-host-check --poll 2000
            xterm -T "StarRocks Admin - Frontend (WSL)" -e bash -c "cd \"$PROJECT_ROOT/frontend\" && echo '=== 验证 WSL 环境 ===' && uname -a && which node && node --version && echo '' && npm start; exec bash" &
            ;;
        tmux)
            echo -e "${YELLOW}在 Tmux 会话 (WSL) 中启动前端...${NC}"
            # package.json 中已经包含了 --host 0.0.0.0 --disable-host-check --poll 2000
            tmux new-session -d -s starrocks-frontend "cd \"$PROJECT_ROOT/frontend\" && echo '=== WSL 环境 ===' && uname -a && which node && node --version && echo '' && npm start"
            echo -e "${GREEN}前端在 tmux 会话 'starrocks-frontend' 中运行${NC}"
            echo -e "${YELLOW}使用 'tmux attach -t starrocks-frontend' 查看${NC}"
            ;;
        screen)
            echo -e "${YELLOW}在 Screen 会话 (WSL) 中启动前端...${NC}"
            # package.json 中已经包含了 --host 0.0.0.0 --disable-host-check --poll 2000
            screen -dmS starrocks-frontend bash -c "cd \"$PROJECT_ROOT/frontend\" && echo '=== WSL 环境 ===' && uname -a && which node && node --version && echo '' && npm start"
            echo -e "${GREEN}前端在 screen 会话 'starrocks-frontend' 中运行${NC}"
            echo -e "${YELLOW}使用 'screen -r starrocks-frontend' 查看${NC}"
            ;;
        *)
            echo -e "${RED}无法在新终端中启动前端${NC}"
            return 1
            ;;
    esac
}

# 检查环境
echo -e "${YELLOW}[1/3]${NC} 检查环境..."
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

# 检测终端类型
echo -e "${YELLOW}[2/3]${NC} 检测终端类型..."
TERMINAL_TYPE=$(detect_terminal)
echo -e "${GREEN}✓ 检测到终端类型: $TERMINAL_TYPE${NC}"
echo ""

# 启动服务
echo -e "${YELLOW}[3/3]${NC} 启动开发服务..."
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo ""

# 启动后端
start_backend_in_terminal "$TERMINAL_TYPE"
sleep 2

# 启动前端
start_frontend_in_terminal "$TERMINAL_TYPE"
sleep 2

echo ""
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo -e "${GREEN}开发环境启动完成！${NC}"
echo ""
echo -e "${BLUE}访问地址:${NC}"
echo -e "  Frontend: ${GREEN}http://localhost:4200${NC}"
echo -e "  Backend:  ${GREEN}http://0.0.0.0:8081${NC}"
echo -e "  API Docs: ${GREEN}http://0.0.0.0:8081/api-docs${NC}"
echo ""

if [ "$TERMINAL_TYPE" = "tmux" ]; then
    echo -e "${YELLOW}终端会话管理:${NC}"
    echo -e "  查看后端: ${GREEN}tmux attach -t starrocks-backend${NC}"
    echo -e "  查看前端: ${GREEN}tmux attach -t starrocks-frontend${NC}"
    echo -e "  列出所有会话: ${GREEN}tmux ls${NC}"
elif [ "$TERMINAL_TYPE" = "screen" ]; then
    echo -e "${YELLOW}终端会话管理:${NC}"
    echo -e "  查看后端: ${GREEN}screen -r starrocks-backend${NC}"
    echo -e "  查看前端: ${GREEN}screen -r starrocks-frontend${NC}"
    echo -e "  列出所有会话: ${GREEN}screen -ls${NC}"
fi

echo ""
echo -e "${YELLOW}服务已在独立的终端窗口中运行${NC}"
echo -e "${YELLOW}每个服务都有自己的控制台，方便查看日志${NC}"
echo ""

# 如果使用后台模式，主进程退出
if [ "$MODE" != "dev" ]; then
    exit 0
fi

# 如果使用终端复用器，等待用户中断
if [ "$TERMINAL_TYPE" = "tmux" ] || [ "$TERMINAL_TYPE" = "screen" ]; then
    echo -e "${YELLOW}按 Ctrl+C 停止所有服务...${NC}"
    trap "bash scripts/dev/stop.sh; exit 0" SIGINT SIGTERM
    # 保持主进程运行
    while true; do
        sleep 1
    done
fi

