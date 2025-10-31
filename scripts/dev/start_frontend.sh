#!/bin/bash

# StarRocks Admin Frontend - 开发环境管理脚本

set -e

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
SHARED_CONFIG="$PROJECT_ROOT/conf/shared.json"
HOST="0.0.0.0"
LOG_FILE="$PROJECT_ROOT/frontend/frontend.log"
PID_FILE="$PROJECT_ROOT/frontend/frontend.pid"

# 从共享配置读取端口（如果可用）
if command -v jq > /dev/null 2>&1 && [ -f "$SHARED_CONFIG" ]; then
    PORT=$(jq -r '.dev.frontend.port' "$SHARED_CONFIG" 2>/dev/null || echo "4200")
else
    PORT="4200"
fi

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 显示帮助信息
show_help() {
    echo -e "${GREEN}StarRocks Admin Frontend 开发环境管理脚本${NC}"
    echo ""
    echo "用法: $0 {start|stop|restart|status|logs|help}"
    echo ""
    echo "命令:"
    echo "  start   - 启动前端服务"
    echo "  stop    - 停止前端服务"
    echo "  restart - 重启前端服务"
    echo "  status  - 查看服务状态"
    echo "  logs    - 查看实时日志"
    echo "  help    - 显示此帮助信息"
    echo ""
}

# 检查服务是否运行
is_running() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            return 0
        else
            rm -f "$PID_FILE"
            return 1
        fi
    fi
    return 1
}

# 获取服务PID
get_pid() {
    if [ -f "$PID_FILE" ]; then
        cat "$PID_FILE"
    else
        echo ""
    fi
}

# 启动服务
start_service() {
    if is_running; then
        echo -e "${YELLOW}[WARNING]${NC} 前端服务已在运行 (PID: $(get_pid))"
        return 0
    fi

    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}StarRocks Admin Frontend 启动脚本${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""

    # 检查Node.js和npm（确保在 WSL 环境中）
    echo -e "${YELLOW}[INFO]${NC} 检查 WSL 环境中的 Node.js..."
    if ! command -v node >/dev/null 2>&1; then
        echo -e "${RED}[ERROR]${NC} Node.js 未在 WSL 中安装"
        echo ""
        
        # 检查是否在交互式终端中
        if [ -t 0 ]; then
            echo -e "${YELLOW}[INFO]${NC} 是否要现在安装 Node.js? (y/N)"
            read -p "> " -n 1 -r
            echo ""
            
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                echo -e "${BLUE}[INFO]${NC} 正在安装 Node.js..."
                bash "$PROJECT_ROOT/scripts/dev/install_nodejs.sh"
                # 重新加载 PATH
                export PATH="$PATH"
            else
                echo -e "${YELLOW}[INFO]${NC} 已跳过安装"
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
            echo -e "${YELLOW}[INFO]${NC} 非交互式模式，无法自动安装"
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

    if ! command -v npm >/dev/null 2>&1; then
        echo -e "${RED}[ERROR]${NC} npm 未在 WSL 中安装，请先安装 npm"
        exit 1
    fi
    
    # 加载公共函数
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    source "$SCRIPT_DIR/common.sh"
    
    # 验证 Node.js 是否在 WSL 中
    if ! verify_wsl_tool "node" "Node.js"; then
        echo -e "${YELLOW}[INFO]${NC} 前端必须在 WSL 中运行，请安装 WSL 版本的 Node.js："
        echo -e "  bash scripts/dev/install_nodejs.sh"
        echo ""
        exit 1
    fi

    # 检查前端目录
    if [ ! -d "$FRONTEND_DIR" ]; then
        echo -e "${RED}[ERROR]${NC} 前端目录不存在: $FRONTEND_DIR"
        exit 1
    fi

    # 检查package.json
    if [ ! -f "$FRONTEND_DIR/package.json" ]; then
        echo -e "${RED}[ERROR]${NC} package.json 不存在: $FRONTEND_DIR/package.json"
        exit 1
    fi

    # 生成前端环境配置文件
    echo -e "${YELLOW}[INFO]${NC} 生成前端环境配置文件..."
    cd "$FRONTEND_DIR"
    if npm run config:generate > /dev/null 2>&1; then
        echo -e "${GREEN}[INFO]${NC} 前端环境配置已生成"
    else
        echo -e "${YELLOW}[WARNING]${NC} 无法自动生成前端环境配置，将使用现有配置"
    fi

    # 安装依赖
    echo -e "${YELLOW}[INFO]${NC} 检查并安装依赖..."
    cd "$FRONTEND_DIR"
    if [ ! -d "node_modules" ]; then
        echo -e "${YELLOW}[INFO]${NC} 安装 npm 依赖..."
        # 确保 .npmrc 存在
        if [ ! -f ".npmrc" ]; then
            echo "legacy-peer-deps=true" > .npmrc
        fi
        npm install
    else
        echo -e "${GREEN}[INFO]${NC} 依赖已存在，跳过安装"
    fi

    # 启动前端服务
    echo -e "${GREEN}[START]${NC} 启动前端服务..."
    echo "  - 主机: $HOST"
    echo "  - 端口: $PORT"
    echo "  - 日志: $LOG_FILE"
    echo ""

    # 检查端口是否被占用
    if lsof -i :$PORT > /dev/null 2>&1; then
        echo -e "${YELLOW}[WARNING]${NC} 端口 $PORT 已被占用，尝试停止占用进程..."
        lsof -ti :$PORT | xargs kill -9 2>/dev/null || true
        sleep 2
    fi

    # 启动Angular开发服务器（在 WSL 中）
    # 注意: package.json 中已经包含了 --host 0.0.0.0 --disable-host-check --poll 2000
    # 如果需要指定端口，可以通过环境变量或修改 package.json
    echo -e "${GREEN}[START]${NC} 在 WSL 中启动前端服务..."
    # 如果端口不是默认的4200，需要通过参数覆盖
    if [ "$PORT" != "4200" ]; then
        nohup npm start -- --port $PORT > "$LOG_FILE" 2>&1 &
    else
        nohup npm start > "$LOG_FILE" 2>&1 &
    fi
    FRONTEND_PID=$!
    echo $FRONTEND_PID > "$PID_FILE"

    # 等待启动
    echo -e "${YELLOW}[INFO]${NC} 等待前端服务启动..."
    sleep 5

    # 检查进程
    if ps -p $FRONTEND_PID > /dev/null 2>&1; then
        echo -e "${GREEN}[SUCCESS]${NC} 前端启动成功!"
        echo "  - PID: $FRONTEND_PID"
        echo "  - 访问地址: http://$HOST:$PORT"
        echo "  - 日志文件: $LOG_FILE"
        echo ""
        
        # 测试服务是否可访问
        if curl -s "http://$HOST:$PORT" > /dev/null 2>&1; then
            echo -e "${GREEN}[健康检查]${NC} ✅ 前端服务可访问"
        else
            echo -e "${YELLOW}[警告]${NC} 前端已启动但可能还在初始化中，请稍等片刻"
        fi
    else
        echo -e "${RED}[ERROR]${NC} 前端启动失败，请查看日志:"
        echo "  tail -f $LOG_FILE"
        rm -f "$PID_FILE"
        exit 1
    fi

    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}前端已启动${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
}

# 停止服务
stop_service() {
    if ! is_running; then
        echo -e "${YELLOW}[WARNING]${NC} 前端服务未运行"
        return 0
    fi

    local pid=$(get_pid)
    echo -e "${YELLOW}[INFO]${NC} 停止前端服务 (PID: $pid)..."
    
    # 优雅停止
    kill -TERM "$pid" 2>/dev/null || true
    sleep 3
    
    # 检查是否还在运行
    if ps -p "$pid" > /dev/null 2>&1; then
        echo -e "${YELLOW}[INFO]${NC} 强制停止前端服务..."
        kill -KILL "$pid" 2>/dev/null || true
        sleep 1
    fi
    
    # 清理PID文件
    rm -f "$PID_FILE"
    
    if ps -p "$pid" > /dev/null 2>&1; then
        echo -e "${RED}[ERROR]${NC} 无法停止前端服务"
        exit 1
    else
        echo -e "${GREEN}[SUCCESS]${NC} 前端服务已停止"
    fi
}

# 重启服务
restart_service() {
    echo -e "${BLUE}[INFO]${NC} 重启前端服务..."
    stop_service
    sleep 2
    start_service
}

# 查看服务状态
show_status() {
    if is_running; then
        local pid=$(get_pid)
        echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行"
        echo "  - PID: $pid"
        echo "  - 访问地址: http://$HOST:$PORT"
        echo "  - 日志文件: $LOG_FILE"
        
        # 测试服务是否可访问
        if curl -s "http://$HOST:$PORT" > /dev/null 2>&1; then
            echo -e "  - 健康状态: ${GREEN}✅ 可访问${NC}"
        else
            echo -e "  - 健康状态: ${RED}❌ 不可访问${NC}"
        fi
    else
        echo -e "${RED}[STATUS]${NC} 前端服务未运行"
    fi
}

# 查看日志
show_logs() {
    if [ ! -f "$LOG_FILE" ]; then
        echo -e "${YELLOW}[WARNING]${NC} 日志文件不存在: $LOG_FILE"
        return 1
    fi
    
    echo -e "${BLUE}[INFO]${NC} 显示实时日志 (按 Ctrl+C 退出)..."
    tail -f "$LOG_FILE"
}

# 主函数
main() {
    case "${1:-start}" in
        start)
            start_service
            ;;
        stop)
            stop_service
            ;;
        restart)
            restart_service
            ;;
        status)
            show_status
            ;;
        logs)
            show_logs
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            echo -e "${RED}[ERROR]${NC} 未知命令: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# 执行主函数
main "$@"