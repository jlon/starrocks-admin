#!/bin/bash

# StarRocks Admin Backend - Development Mode with Hot Reload
# 开发模式后端启动脚本（支持热重载）

set -e

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
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
echo -e "${BLUE}StarRocks Admin Backend - Dev Mode${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 检查 cargo-watch 是否安装
echo -e "${YELLOW}[1/3]${NC} 检查开发工具..."
if ! cargo --list | grep -q "watch"; then
    echo -e "${YELLOW}cargo-watch 未安装，正在安装...${NC}"
    cargo install cargo-watch
else
    echo -e "${GREEN}✓ cargo-watch 已安装${NC}"
fi
echo ""

# 创建必要的目录
echo -e "${YELLOW}[2/3]${NC} 准备开发环境..."
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

# 启动开发服务器（热重载模式）
echo -e "${BLUE}[3/3]${NC} 启动后端（热重载模式）..."
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo -e "服务启动后，代码修改将自动重新编译并重启"
echo -e "按 Ctrl+C 停止服务"
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo ""

cd "$BACKEND_DIR"

# 使用 cargo-watch 运行开发模式，监听文件变化
cargo watch -x "run"

