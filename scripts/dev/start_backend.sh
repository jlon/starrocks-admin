#!/bin/bash

# StarRocks Admin Backend - 开发环境管理脚本

set -e

# Initialize Rust environment for non-interactive shell
# This is needed because the script runs in a non-interactive bash context
# and doesn't load ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/backend"
CONFIG_DIR="$PROJECT_ROOT/backend/conf"
DB_DIR="${DB_DIR:-$PROJECT_ROOT/backend/data}"
LOG_DIR="${LOG_DIR:-$PROJECT_ROOT/backend/logs}"
PID_FILE="$PROJECT_ROOT/backend/starrocks-admin.pid"

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 显示帮助信息
show_help() {
    echo -e "${GREEN}StarRocks Admin Backend 开发环境管理脚本${NC}"
    echo ""
    echo "用法: $0 {start|stop|restart|status|logs|help}"
    echo ""
    echo "命令:"
    echo "  start   - 启动后端服务"
    echo "  stop    - 停止后端服务"
    echo "  restart - 重启后端服务"
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
        echo -e "${YELLOW}[WARNING]${NC} 后端服务已在运行 (PID: $(get_pid))"
        return 0
    fi

    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}StarRocks Admin Backend 启动脚本${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""

    # 创建必要的目录
    echo -e "${YELLOW}[INFO]${NC} 创建必要的目录..."
    mkdir -p "$DB_DIR"
    mkdir -p "$LOG_DIR"
    mkdir -p "$CONFIG_DIR"

    # 创建开发环境配置文件
    echo -e "${YELLOW}[INFO]${NC} 重新创建开发环境配置文件..."
    rm -f "$CONFIG_DIR/config.toml"
    cat > "$CONFIG_DIR/config.toml" <<EOF
[server]
host = "0.0.0.0"
port = 8081

[database]
url = "sqlite://$DB_DIR/starrocks-admin.db"

[auth]
jwt_secret = "dev-secret-key-change-in-production"
jwt_expires_in = "24h"

[cors]
allow_origin = "http://0.0.0.0:4200"

[logging]
level = "debug"
file = "logs/starrocks-admin.log"
EOF
    echo -e "${GREEN}[INFO]${NC} 配置文件已创建: $CONFIG_DIR/config.toml"
    echo -e "${GREEN}[INFO]${NC} 数据库路径: $DB_DIR/starrocks-admin.db"

    # 强制重新编译以确保使用最新代码
    echo -e "${YELLOW}[BUILD]${NC} 编译最新代码..."
    cd "$BACKEND_DIR"
    cargo build --release
    echo -e "${GREEN}[BUILD]${NC} 编译完成"
    echo ""

    # 显示配置信息
    echo -e "${GREEN}[CONFIG]${NC} 配置信息:"
    echo "  - 配置文件: $CONFIG_DIR/config.toml"
    echo "  - 数据库目录: $DB_DIR"
    echo "  - 日志目录: $LOG_DIR"
    echo "  - 工作目录: $BACKEND_DIR"
    echo ""

    # 检查端口是否被占用
    PORT=8081
    if lsof -i :$PORT > /dev/null 2>&1; then
        echo -e "${YELLOW}[WARNING]${NC} 端口 $PORT 已被占用，尝试停止占用进程..."
        lsof -ti :$PORT | xargs kill -9 2>/dev/null || true
        sleep 2
    fi

    # 启动后端
    echo -e "${GREEN}[START]${NC} 启动后端服务..."
    cd "$BACKEND_DIR"
    nohup ./target/release/starrocks-admin > "$LOG_DIR/starrocks-admin.log" 2>&1 &
    BACKEND_PID=$!
    echo $BACKEND_PID > "$PID_FILE"

    # 等待启动
    sleep 3

    # 检查进程
    if ps -p $BACKEND_PID > /dev/null; then
        echo -e "${GREEN}[SUCCESS]${NC} 后端启动成功!"
        echo "  - PID: $BACKEND_PID"
        echo "  - 健康检查: http://localhost:8081/health"
        echo "  - Web UI: http://localhost:8081"
        echo ""
        
        # 测试健康检查
        if curl -s "http://localhost:8081/health" > /dev/null 2>&1; then
            echo -e "${GREEN}[健康检查]${NC} ✅ Backend运行正常"
        else
            echo -e "${YELLOW}[警告]${NC} Backend已启动但健康检查失败，请查看日志"
        fi
    else
        echo -e "${RED}[ERROR]${NC} 后端启动失败，请查看日志:"
        echo "  tail -f $LOG_DIR/starrocks-admin.log"
        rm -f "$PID_FILE"
        exit 1
    fi

    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}后端已启动${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
}

# 停止服务
stop_service() {
    if ! is_running; then
        echo -e "${YELLOW}[WARNING]${NC} 后端服务未运行"
        return 0
    fi

    local pid=$(get_pid)
    echo -e "${YELLOW}[INFO]${NC} 停止后端服务 (PID: $pid)..."
    
    # 优雅停止
    kill -TERM "$pid" 2>/dev/null || true
    sleep 2
    
    # 检查是否还在运行
    if ps -p "$pid" > /dev/null 2>&1; then
        echo -e "${YELLOW}[INFO]${NC} 强制停止后端服务..."
        kill -KILL "$pid" 2>/dev/null || true
        sleep 1
    fi
    
    # 清理PID文件
    rm -f "$PID_FILE"
    
    if ps -p "$pid" > /dev/null 2>&1; then
        echo -e "${RED}[ERROR]${NC} 无法停止后端服务"
        exit 1
    else
        echo -e "${GREEN}[SUCCESS]${NC} 后端服务已停止"
    fi
}

# 重启服务
restart_service() {
    echo -e "${BLUE}[INFO]${NC} 重启后端服务..."
    stop_service
    sleep 1
    start_service
}

# 查看服务状态
show_status() {
    if is_running; then
        local pid=$(get_pid)
        echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行"
        echo "  - PID: $pid"
        echo "  - 配置文件: $CONFIG_DIR/config.toml"
        echo "  - 日志文件: $LOG_DIR/starrocks-admin.log"
        echo "  - 健康检查: http://localhost:8081/health"
        echo "  - Web UI: http://localhost:8081"
        
        # 测试健康检查
        if curl -s "http://localhost:8081/health" > /dev/null 2>&1; then
            echo -e "  - 健康状态: ${GREEN}✅ 正常${NC}"
        else
            echo -e "  - 健康状态: ${RED}❌ 异常${NC}"
        fi
    else
        echo -e "${RED}[STATUS]${NC} 后端服务未运行"
    fi
}

# 查看日志
show_logs() {
    if [ ! -f "$LOG_DIR/starrocks-admin.log" ]; then
        echo -e "${YELLOW}[WARNING]${NC} 日志文件不存在: $LOG_DIR/starrocks-admin.log"
        return 1
    fi
    
    echo -e "${BLUE}[INFO]${NC} 显示实时日志 (按 Ctrl+C 退出)..."
    tail -f "$LOG_DIR/starrocks-admin.log"
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