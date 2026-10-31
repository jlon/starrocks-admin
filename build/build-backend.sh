#!/usr/bin/env bash

#
# StarRocks Admin - Backend Build Script
# Builds the Rust backend and outputs to build/dist/
# Backend embeds frontend assets directly from frontend/dist/
#

set -e

# Get project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/backend"
BUILD_DIR="$PROJECT_ROOT/build"
DIST_DIR="$BUILD_DIR/dist"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Building StarRocks Admin Backend${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Create dist directories
echo -e "${YELLOW}[0/4]${NC} Creating build directories..."
mkdir -p "$DIST_DIR/bin"
mkdir -p "$DIST_DIR/conf"
mkdir -p "$DIST_DIR/lib"
mkdir -p "$DIST_DIR/data"
mkdir -p "$DIST_DIR/logs"
mkdir -p "$DIST_DIR/migrations"

# Clean old backend artifacts
echo -e "${YELLOW}[0/4]${NC} Cleaning old backend artifacts..."
rm -f "$DIST_DIR/bin/"*
rm -f "$DIST_DIR/conf/"*
rm -f "$DIST_DIR/lib/"*
rm -f "$DIST_DIR/migrations/"*

# Build backend
echo -e "${YELLOW}[1/4]${NC} Compiling Rust backend (release mode)..."
cd "$BACKEND_DIR"
cargo build --release

# Copy binary
echo -e "${YELLOW}[2/4]${NC} Copying backend binary..."
cp target/release/starrocks-admin "$DIST_DIR/bin/"

# Create production configuration file
echo -e "${YELLOW}[3/4]${NC} Creating production configuration file..."
cat > "$DIST_DIR/conf/config.toml" << 'EOF'
[server]
host = "0.0.0.0"
port = 8080

[database]
url = "sqlite://data/starrocks-admin.db"

[auth]
jwt_secret = "dev-secret-key-change-in-production"
jwt_expires_in = "24h"

[logging]
level = "info,starrocks_admin_backend=debug"
file = "logs/starrocks-admin.log"


# Metrics collector configuration
[metrics]
interval_secs = "30s"    
retention_days = "7d"    
enabled = true

# Audit log configuration
[audit]
database = "starrocks_audit_db__"
table = "starrocks_audit_tbl__"
EOF
echo "Created production config.toml"

# Copy migrations
echo -e "${YELLOW}[4/4]${NC} Copying database migrations..."
if [ -d "$BACKEND_DIR/migrations" ]; then
    cp -r "$BACKEND_DIR/migrations"/* "$DIST_DIR/migrations/"
    echo "Copied $(ls "$DIST_DIR/migrations" | wc -l) migration files"
else
    echo "Warning: No migrations directory found in backend"
fi

# Create enhanced start script for backend
cat > "$DIST_DIR/bin/starrocks-admin.sh" << 'EOF'
#!/bin/bash

# StarRocks Admin Backend - 生产环境管理脚本

set -e

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY_PATH="$SCRIPT_DIR/starrocks-admin"
CONFIG_DIR="$DIST_ROOT/conf"
DATA_DIR="$DIST_ROOT/data"
LOG_DIR="$DIST_ROOT/logs"
PID_FILE="$DIST_ROOT/starrocks-admin.pid"

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 显示帮助信息
show_help() {
    echo -e "${GREEN}StarRocks Admin Backend 生产环境管理脚本${NC}"
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
        local pid=$(get_pid)
        local started_at=$(get_recorded_start_time)
        if is_our_process "$pid" "$started_at"; then
            return 0
        else
            rm -f "$PID_FILE"
            return 1
        fi
    fi
    return 1
}

# PID 文件可能过期；核对程序路径和进程启动时间，避免 PID 复用造成误停。
is_our_process() {
    local pid=$1
    local expected_start_time=${2:-}
    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [ -r "/proc/$pid/exe" ] || return 1
    [ "$(readlink "/proc/$pid/exe")" = "$(readlink -f "$BINARY_PATH")" ] || return 1
    [ -z "$expected_start_time" ] || [ "$(process_start_time "$pid")" = "$expected_start_time" ]
}

process_start_time() {
    local pid=$1
    local stat_line
    stat_line=$(cat "/proc/$pid/stat" 2>/dev/null) || return 1
    stat_line=${stat_line##*) }
    awk '{print $20}' <<< "$stat_line"
}

# 获取服务PID
get_pid() {
    if [ -f "$PID_FILE" ]; then
        awk '{print $1}' "$PID_FILE"
    else
        echo ""
    fi
}

get_recorded_start_time() {
    if [ -f "$PID_FILE" ]; then
        awk '{print $2}' "$PID_FILE"
    fi
}

# 读取与后端一致的监听配置；APP_SERVER_* 优先于 config.toml。
server_host() {
    if [ -n "${APP_SERVER_HOST:-}" ]; then
        printf '%s' "$APP_SERVER_HOST"
    elif [ -f "$CONFIG_DIR/config.toml" ]; then
        awk '/^[[:space:]]*\[server\][[:space:]]*$/ { in_server=1; next }
             /^[[:space:]]*\[/ { in_server=0 }
             in_server && /^[[:space:]]*host[[:space:]]*=/ {
                 sub(/^[^=]*=[[:space:]]*/, ""); sub(/[[:space:]#].*$/, "");
                 gsub(/^["'\''[:space:]]+|["'\''[:space:]]+$/, ""); print; exit
             }' "$CONFIG_DIR/config.toml"
    else
        printf '0.0.0.0'
    fi
}

server_port() {
    if [ -n "${APP_SERVER_PORT:-}" ]; then
        printf '%s' "$APP_SERVER_PORT"
    elif [ -f "$CONFIG_DIR/config.toml" ]; then
        awk '/^[[:space:]]*\[server\][[:space:]]*$/ { in_server=1; next }
             /^[[:space:]]*\[/ { in_server=0 }
             in_server && /^[[:space:]]*port[[:space:]]*=/ {
                 sub(/^[^=]*=[[:space:]]*/, ""); sub(/[[:space:]#].*$/, ""); print; exit
             }' "$CONFIG_DIR/config.toml"
    else
        printf '8080'
    fi
}

# 通配地址只能用于监听，不能作为可靠的客户端访问地址。
health_check_host() {
    case "$1" in
        0.0.0.0) printf '127.0.0.1' ;;
        ::|'[::]') printf '::1' ;;
        *) printf '%s' "$1" ;;
    esac
}

web_ui_host() {
    case "$1" in
        0.0.0.0|::|'[::]') printf 'localhost' ;;
        *) printf '%s' "$1" ;;
    esac
}

url_host() {
    local host=$1
    host=${host#[}
    host=${host%]}
    case "$1" in
        *:*) printf '[%s]' "$host" ;;
        *) printf '%s' "$host" ;;
    esac
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

    # 检查二进制文件
    if [ ! -f "$BINARY_PATH" ]; then
        echo -e "${RED}[ERROR]${NC} 二进制文件不存在: $BINARY_PATH"
        exit 1
    fi

    # 创建必要的目录
    echo -e "${YELLOW}[INFO]${NC} 创建必要的目录..."
    mkdir -p "$DATA_DIR"
    mkdir -p "$LOG_DIR"
    mkdir -p "$CONFIG_DIR"

    # 设置环境变量
    export DATABASE_URL="${DATABASE_URL:-sqlite://$DATA_DIR/starrocks-admin.db}"
    local host=$(server_host)
    host=${host:-0.0.0.0}
    local port=$(server_port)
    port=${port:-8080}
    local listen_host=$(url_host "$host")
    local check_host=$(url_host "$(health_check_host "$host")")
    local display_host=$(url_host "$(web_ui_host "$host")")

    # 显示配置信息
    echo -e "${GREEN}[CONFIG]${NC} 配置信息:"
    echo "  - 二进制文件: $BINARY_PATH"
    echo "  - 配置文件: $CONFIG_DIR/config.toml"
    echo "  - 数据目录: $DATA_DIR"
    echo "  - 日志目录: $LOG_DIR"
    echo "  - 监听地址: $listen_host:$port"
    echo "  - 数据库: $DATABASE_URL"
    echo ""

    # 启动后端
    echo -e "${GREEN}[START]${NC} 启动后端服务..."
    cd "$DIST_ROOT"
    nohup "$BINARY_PATH" > "$LOG_DIR/starrocks-admin.log" 2>&1 &
    BACKEND_PID=$!

    # 等待启动
    sleep 3

    # 检查进程
    if is_our_process "$BACKEND_PID"; then
        printf '%s %s\n' "$BACKEND_PID" "$(process_start_time "$BACKEND_PID")" > "$PID_FILE"
        echo -e "${GREEN}[SUCCESS]${NC} 后端启动成功!"
        echo "  - PID: $BACKEND_PID"
        echo "  - 健康检查: http://$check_host:$port/health"
        echo "  - Web UI: http://$display_host:$port"
echo ""

        # 测试健康检查
        if curl -fsS "http://$check_host:$port/health" > /dev/null 2>&1; then
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
    local started_at=$(get_recorded_start_time)
    echo -e "${YELLOW}[INFO]${NC} 停止后端服务 (PID: $pid)..."
    
    # 优雅停止
    kill -TERM "$pid" 2>/dev/null || true
    sleep 2
    
    # 检查是否还在运行
    if is_our_process "$pid" "$started_at"; then
        echo -e "${YELLOW}[INFO]${NC} 强制停止后端服务..."
        kill -KILL "$pid" 2>/dev/null || true
        sleep 1
    fi
    
    # 清理PID文件
    rm -f "$PID_FILE"
    
    if is_our_process "$pid" "$started_at"; then
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
        local host=$(server_host)
        host=${host:-0.0.0.0}
        local port=$(server_port)
        port=${port:-8080}
        local check_host=$(url_host "$(health_check_host "$host")")
        local display_host=$(url_host "$(web_ui_host "$host")")
        echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行"
        echo "  - PID: $pid"
        echo "  - 二进制文件: $BINARY_PATH"
        echo "  - 配置文件: $CONFIG_DIR/config.toml"
        echo "  - 日志文件: $LOG_DIR/starrocks-admin.log"
        echo "  - 数据目录: $DATA_DIR"
        echo "  - 健康检查: http://$check_host:$port/health"
        echo "  - Web UI: http://$display_host:$port"
        
        # 测试健康检查
        if curl -fsS "http://$check_host:$port/health" > /dev/null 2>&1; then
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
EOF

chmod +x "$DIST_DIR/bin/starrocks-admin.sh"

echo ""
echo -e "${GREEN}✓ Backend build complete!${NC}"
echo -e "  Binary: $DIST_DIR/bin/starrocks-admin"
echo -e "  Startup script: $DIST_DIR/bin/starrocks-admin.sh"
echo -e "  Config file: $DIST_DIR/conf/config.toml"
echo -e "  Note: Frontend assets are embedded directly from frontend/dist/"
