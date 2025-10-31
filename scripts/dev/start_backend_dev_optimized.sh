#!/bin/bash

# StarRocks Admin Backend - Optimized Development Mode
# 优化的开发模式后端启动脚本（支持增量编译，避免重复编译第三方库）

set -e

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# 加载 Rust 环境（如果存在）
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# 加载公共函数
source "$SCRIPT_DIR/common.sh"
BACKEND_DIR="$PROJECT_ROOT/backend"
CONFIG_DIR="$PROJECT_ROOT/backend/conf"
DB_DIR="${DB_DIR:-$PROJECT_ROOT/backend/data}"
LOG_DIR="${LOG_DIR:-$PROJECT_ROOT/backend/logs}"

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}StarRocks Admin Backend - Optimized Dev${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 验证当前在 WSL 环境中
echo -e "${YELLOW}[0/4]${NC} 验证运行环境..."
print_wsl_status || echo -e "${YELLOW}继续尝试启动...${NC}"

# 验证 Rust 工具链在 WSL 中
echo -e "${YELLOW}验证 Rust 工具链...${NC}"
if ! verify_wsl_tool "cargo" "Cargo"; then
    echo -e "${YELLOW}请确保 Rust 工具链已安装在 WSL 中${NC}"
    exit 1
fi

# 检查 cargo-watch 是否安装
echo -e "${YELLOW}[1/4]${NC} 检查开发工具..."
if ! cargo --list | grep -q "watch"; then
    echo -e "${YELLOW}cargo-watch 未安装，正在安装...${NC}"
    cargo install cargo-watch
else
    echo -e "${GREEN}✓ cargo-watch 已安装${NC}"
fi
echo ""

# 创建必要的目录
echo -e "${YELLOW}[2/4]${NC} 准备开发环境..."
mkdir -p "$DB_DIR"
mkdir -p "$LOG_DIR"
mkdir -p "$CONFIG_DIR"

# 创建开发环境配置文件
rm -f "$CONFIG_DIR/config.toml"
cat > "$CONFIG_DIR/config.toml" << 'EOF'
[server]
host = "0.0.0.0"
port = 8081

[database]
url = "sqlite:///tmp/starrocks-admin.db"

[auth]
jwt_secret = "dev-secret-key-change-in-production"
jwt_expires_in = "24h"

[cors]
allow_origin = "http://0.0.0.0:4200"

[logging]
level = "debug"
file = "logs/starrocks-admin.log"

[static_config]
enabled = false
web_root = "../build/dist/web"
EOF
echo -e "${GREEN}✓ 配置文件已创建${NC}"
echo ""

# 设置编译优化环境变量
# 根据 CPU 核心数设置并行任务数（留一些核心给系统和其他进程）
CPU_CORES=$(nproc 2>/dev/null || echo "8")
# 使用约 75% 的核心数，但至少 4 个，最多 14 个
PARALLEL_JOBS=$((CPU_CORES * 3 / 4))
if [ $PARALLEL_JOBS -lt 4 ]; then
    PARALLEL_JOBS=4
elif [ $PARALLEL_JOBS -gt 14 ]; then
    PARALLEL_JOBS=14
fi

export CARGO_BUILD_JOBS=$PARALLEL_JOBS
export CARGO_INCREMENTAL=1
export RUSTC_WRAPPER=""  # 如果使用 sccache，会在这里设置

echo -e "${BLUE}[INFO]${NC} 编译优化配置:"
echo -e "  - 并行任务数: $PARALLEL_JOBS (CPU 核心数: $CPU_CORES)"
echo -e "  - 增量编译: 启用"
echo ""

# 检查是否需要首次编译
echo -e "${YELLOW}[3/4]${NC} 检查编译状态..."
cd "$BACKEND_DIR"

# 检查是否存在编译缓存
if [ ! -d "target/debug" ] || [ ! -f "target/debug/deps/libstarrocks_admin-*.rlib" ]; then
    echo -e "${YELLOW}首次编译或缓存缺失，进行完整编译...${NC}"
    echo -e "${BLUE}这可能需要几分钟时间，请耐心等待...${NC}"
    
    # 使用 release 模式编译依赖，debug 模式编译项目代码
    # 这样可以获得更好的性能，同时保持开发时的快速编译
    cargo build --release --dependencies-only 2>/dev/null || true
    cargo build
    echo -e "${GREEN}✓ 编译完成${NC}"
else
    echo -e "${GREEN}✓ 编译缓存存在，跳过完整编译${NC}"
fi
echo ""

# 启动开发服务器（优化的热重载模式）
echo -e "${BLUE}[4/4]${NC} 启动后端（优化热重载模式）..."
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo -e "优化特性："
echo -e "  • 第三方库已预编译，不会重复编译"
echo -e "  • 只监听项目源代码变化"
echo -e "  • 使用增量编译，速度更快"
echo -e "  • 代码修改将自动重新编译并重启"
echo -e "按 Ctrl+C 停止服务"
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo ""

# 检测是否在 Windows 文件系统上（/mnt/），需要使用轮询模式
# WSL 在 Windows 文件系统上无法使用 inotify，需要使用轮询
cd "$BACKEND_DIR"
USE_POLLING=false
if [[ "$(pwd)" == /mnt/* ]] || [[ "$PWD" == /mnt/* ]]; then
    USE_POLLING=true
    echo -e "${YELLOW}[INFO]${NC} 检测到 Windows 文件系统 (/mnt/)，启用轮询模式以支持文件监听"
    echo -e "${YELLOW}[INFO]${NC} 轮询模式会每2秒检查一次文件变化（性能略有影响但可以正常工作）"
fi

# 使用优化的 cargo-watch 配置
# 只监听源代码文件，排除所有编译输出和日志目录
WATCH_ARGS=(
    --watch "src"
    --watch "Cargo.toml"
    --ignore "target/**"
    --ignore "logs/**"
    --ignore "data/**"
    --ignore "conf/**"
    --ignore "*.log"
    --ignore "*.pid"
    --ignore "*.tmp"
    --ignore "*.swp"
    --ignore "*.swo"
    --ignore ".git/**"
    --ignore "node_modules/**"
    --ignore "dist/**"
    --ignore "build/**"
    --ignore ".angular/**"
    --ignore "coverage/**"
    --ignore "*.lock"
    --ignore "*.orig"
    --ignore "*.rej"
    --delay 1
    --clear
    -x "run --bin starrocks-admin"
)

# 如果在 Windows 文件系统上，添加轮询选项
if [ "$USE_POLLING" = "true" ]; then
    # cargo-watch 在较新版本中支持 --poll 选项
    # 如果版本不支持，使用环境变量 CARGO_WATCH_POLL_INTERVAL
    export CARGO_WATCH_POLL_INTERVAL=2
    WATCH_ARGS+=(--poll)
    echo -e "${BLUE}[INFO]${NC} 使用轮询模式（每2秒检查一次文件变化）"
fi

echo -e "${BLUE}[INFO]${NC} 启动热重载监听..."
echo -e "${BLUE}[INFO]${NC} 修改任何 .rs 文件将自动触发重新编译并重启"
echo -e "${BLUE}[INFO]${NC} 编译将使用 $PARALLEL_JOBS 个并行任务"
echo ""

# 确保环境变量被 cargo watch 继承
export CARGO_BUILD_JOBS=$PARALLEL_JOBS
export CARGO_INCREMENTAL=1

cargo watch "${WATCH_ARGS[@]}"
