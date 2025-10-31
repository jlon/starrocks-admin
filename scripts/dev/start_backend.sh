#!/bin/bash

# StarRocks Admin Backend - 开发环境管理脚本

set -e

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/backend"
CONFIG_DIR="$PROJECT_ROOT/backend/conf"
SHARED_CONFIG="$PROJECT_ROOT/conf/shared.json"
DEV_CONFIG_DIR="$PROJECT_ROOT/conf/dev"
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

    # 加载 Rust 环境（如果存在）
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    fi
    
    # 加载公共函数
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    source "$SCRIPT_DIR/common.sh"
    
    # 验证当前在 WSL 环境中
    echo -e "${YELLOW}[INFO]${NC} 验证运行环境..."
    print_wsl_status || echo -e "${YELLOW}[WARNING]${NC} 继续尝试启动..."

    # 验证 Rust 工具链在 WSL 中
    echo -e "${YELLOW}[INFO]${NC} 验证 Rust 工具链..."
    if ! verify_wsl_tool "cargo" "Cargo"; then
        echo -e "${YELLOW}[INFO]${NC} 请确保 Rust 工具链已安装在 WSL 中"
        exit 1
    fi

    # 创建必要的目录
    echo -e "${YELLOW}[INFO]${NC} 创建必要的目录..."
    mkdir -p "$DB_DIR"
    mkdir -p "$LOG_DIR"
    mkdir -p "$CONFIG_DIR"

    # 使用统一的配置文件
    echo -e "${YELLOW}[INFO]${NC} 使用统一配置文件..."
    
    # 检查共享配置文件是否存在
    if [ ! -f "$SHARED_CONFIG" ]; then
        echo -e "${RED}[ERROR]${NC} 共享配置文件不存在: $SHARED_CONFIG"
        exit 1
    fi
    
    # 检查开发环境配置文件是否存在
    if [ ! -f "$DEV_CONFIG_DIR/config.toml" ]; then
        echo -e "${RED}[ERROR]${NC} 开发环境配置文件不存在: $DEV_CONFIG_DIR/config.toml"
        echo -e "${YELLOW}[INFO]${NC} 请确保 conf/dev/config.toml 文件存在"
        exit 1
    fi
    
    # 复制开发环境配置文件到后端配置目录
    cp "$DEV_CONFIG_DIR/config.toml" "$CONFIG_DIR/config.toml"
    echo -e "${GREEN}[INFO]${NC} 配置文件已复制: $CONFIG_DIR/config.toml"
    
    # 从共享配置读取端口信息（用于显示）
    if command -v jq > /dev/null 2>&1; then
        BACKEND_PORT=$(jq -r '.dev.backend.port' "$SHARED_CONFIG" 2>/dev/null || echo "8081")
    else
        BACKEND_PORT="8081"
    fi

    # 检查是否需要编译（避免文件锁冲突）
    echo -e "${YELLOW}[BUILD]${NC} 检查编译状态..."
    cd "$BACKEND_DIR"
    
    # 检查可执行文件是否存在且是最新的
    local needs_build=false
    if [ ! -f "target/release/starrocks-admin" ]; then
        needs_build=true
        echo -e "${YELLOW}可执行文件不存在，需要编译...${NC}"
    elif [ "src" -nt "target/release/starrocks-admin" ] 2>/dev/null || \
         [ "Cargo.toml" -nt "target/release/starrocks-admin" ] 2>/dev/null; then
        needs_build=true
        echo -e "${YELLOW}检测到代码更新，需要重新编译...${NC}"
    else
        echo -e "${GREEN}✓ 可执行文件已存在且为最新，跳过编译${NC}"
    fi
    
    # 如果需要编译，尝试编译（如果遇到锁会自动等待）
    if [ "$needs_build" = "true" ]; then
        echo -e "${YELLOW}开始编译...${NC}"
        # 设置超时，如果30秒内无法获得锁则报错
        # cargo 自己会处理文件锁等待，但我们可以添加超时保护
        timeout 300 cargo build --release || {
            local exit_code=$?
            if [ $exit_code -eq 124 ]; then
                echo -e "${RED}[ERROR]${NC} 编译超时（5分钟），可能因文件锁问题"
                echo -e "${YELLOW}[提示]${NC} 请检查是否有其他 cargo 进程正在运行："
                echo -e "  ps aux | grep cargo"
                echo -e "  或运行: make dev-stop"
                exit 1
            else
                echo -e "${RED}[ERROR]${NC} 编译失败，退出码: $exit_code"
                exit $exit_code
            fi
        }
        echo -e "${GREEN}[BUILD]${NC} 编译完成"
    fi
    echo ""

    # 显示配置信息
    echo -e "${GREEN}[CONFIG]${NC} 配置信息:"
    echo "  - 配置文件: $CONFIG_DIR/config.toml"
    echo "  - 数据库目录: $DB_DIR"
    echo "  - 日志目录: $LOG_DIR"
    echo "  - 工作目录: $BACKEND_DIR"
    echo ""

    # 启动后端（设置环境变量）
    echo -e "${GREEN}[START]${NC} 启动后端服务..."
    cd "$BACKEND_DIR"
    export APP_ENV=dev
    nohup ./target/release/starrocks-admin > "$LOG_DIR/starrocks-admin.log" 2>&1 &
    BACKEND_PID=$!
    echo $BACKEND_PID > "$PID_FILE"

    # 等待启动
    sleep 3

    # 检查进程
    if ps -p $BACKEND_PID > /dev/null; then
        echo -e "${GREEN}[SUCCESS]${NC} 后端启动成功!"
        echo "  - PID: $BACKEND_PID"
        echo "  - 健康检查: http://localhost:${BACKEND_PORT}/health"
        echo "  - Web UI: http://localhost:${BACKEND_PORT}"
        echo ""
        
        # 测试健康检查
        if curl -s "http://localhost:${BACKEND_PORT}/health" > /dev/null 2>&1; then
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
        
        # 从共享配置读取端口
        if command -v jq > /dev/null 2>&1 && [ -f "$SHARED_CONFIG" ]; then
            BACKEND_PORT=$(jq -r '.dev.backend.port' "$SHARED_CONFIG" 2>/dev/null || echo "8081")
        else
            BACKEND_PORT="8081"
        fi
        
        echo "  - 健康检查: http://localhost:${BACKEND_PORT}/health"
        echo "  - Web UI: http://localhost:${BACKEND_PORT}"
        
        # 测试健康检查
        if curl -s "http://localhost:${BACKEND_PORT}/health" > /dev/null 2>&1; then
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