#!/usr/bin/env bash

#
# StarRocks Admin - Optimized Frontend Build Script
# 优化的前端构建脚本（支持增量构建）
#

set -e

# 导入构建工具函数
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/build-utils.sh"

# 获取项目路径
PROJECT_ROOT=$(get_project_root)
FRONTEND_DIR="$PROJECT_ROOT/frontend"
BUILD_DIR="$PROJECT_ROOT/build"
DIST_DIR="$BUILD_DIR/dist"
LAST_BUILD_FILE="$BUILD_DIR/.last_frontend_build"

# 检查是否需要构建
NEEDS_BUILD=$(check_frontend_changed "$PROJECT_ROOT")

if [ "$NEEDS_BUILD" = "false" ]; then
    show_build_status "前端" "false" "文件无修改"
    exit 0
fi

show_build_status "前端" "true" "检测到文件修改"

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Building StarRocks Admin Frontend${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# 检查构建依赖
if ! check_build_dependencies; then
    exit 1
fi

# 创建 web 目录
mkdir -p "$DIST_DIR/web"

# 安装依赖
echo -e "${YELLOW}[1/3]${NC} 安装前端依赖..."
cd "$FRONTEND_DIR"
npm install

# 构建前端
echo -e "${YELLOW}[2/3]${NC} 构建 Angular 前端 (production 模式)..."
npm run build -- --configuration production

# 复制构建文件
echo -e "${YELLOW}[3/3]${NC} 复制前端构建文件..."
cp -r dist/* "$DIST_DIR/web/"

# 更新构建时间戳
update_build_timestamp "$LAST_BUILD_FILE"

echo ""
echo -e "${GREEN}✓ 前端构建完成!${NC}"
echo -e "  输出目录: $DIST_DIR/web/"
