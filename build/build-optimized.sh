#!/usr/bin/env bash

#
# StarRocks Admin - Optimized Build Script
# 优化的构建脚本（支持增量构建）
#

set -e

# 导入构建工具函数
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/build-utils.sh"

# 获取项目路径
PROJECT_ROOT=$(get_project_root)
BUILD_DIR="$PROJECT_ROOT/build"
DIST_DIR="$BUILD_DIR/dist"

# 解析命令行参数
FORCE_BUILD=false
CLEAN_BUILD=false
SHOW_HELP=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --force|-f)
            FORCE_BUILD=true
            shift
            ;;
        --clean|-c)
            CLEAN_BUILD=true
            shift
            ;;
        --help|-h)
            SHOW_HELP=true
            shift
            ;;
        *)
            echo "未知参数: $1"
            echo "使用 --help 查看帮助信息"
            exit 1
            ;;
    esac
done

# 显示帮助信息
if [ "$SHOW_HELP" = "true" ]; then
    echo -e "${BLUE}StarRocks Admin - 优化构建脚本${NC}"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  --force, -f    强制构建（忽略文件修改检测）"
    echo "  --clean, -c    清理构建缓存后构建"
    echo "  --help, -h     显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  $0              # 增量构建（仅构建有修改的组件）"
    echo "  $0 --force      # 强制构建所有组件"
    echo "  $0 --clean      # 清理缓存后构建"
    echo ""
    exit 0
fi

echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   StarRocks Admin - 优化构建脚本      ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# 检查构建依赖
echo -e "${YELLOW}[1/4]${NC} 检查构建依赖..."
if ! check_build_dependencies; then
    exit 1
fi
echo -e "${GREEN}✓${NC} 构建依赖检查通过"
echo ""

# 清理构建缓存（如果需要）
if [ "$CLEAN_BUILD" = "true" ]; then
    echo -e "${YELLOW}[2/4]${NC} 清理构建缓存..."
    clean_build_cache "$PROJECT_ROOT"
    echo ""
else
    echo -e "${YELLOW}[2/4]${NC} 跳过缓存清理"
    echo ""
fi

# 检查构建需求
echo -e "${YELLOW}[3/4]${NC} 检查构建需求..."

# 检查后端构建需求
if [ "$FORCE_BUILD" = "true" ]; then
    BACKEND_NEEDS_BUILD="true"
    show_build_status "后端" "true" "强制构建"
else
    BACKEND_NEEDS_BUILD=$(check_backend_changed "$PROJECT_ROOT")
    if [ "$BACKEND_NEEDS_BUILD" = "true" ]; then
        show_build_status "后端" "true" "检测到文件修改"
    else
        show_build_status "后端" "false" "文件无修改"
    fi
fi

# 检查前端构建需求
if [ "$FORCE_BUILD" = "true" ]; then
    FRONTEND_NEEDS_BUILD="true"
    show_build_status "前端" "true" "强制构建"
else
    FRONTEND_NEEDS_BUILD=$(check_frontend_changed "$PROJECT_ROOT")
    if [ "$FRONTEND_NEEDS_BUILD" = "true" ]; then
        show_build_status "前端" "true" "检测到文件修改"
    else
        show_build_status "前端" "false" "文件无修改"
    fi
fi

# 如果都不需要构建，直接退出
if [ "$BACKEND_NEEDS_BUILD" = "false" ] && [ "$FRONTEND_NEEDS_BUILD" = "false" ]; then
    echo ""
    echo -e "${GREEN}✓${NC} 所有组件都无需构建，构建完成！"
    exit 0
fi

echo ""

# 构建后端（如果需要）
if [ "$BACKEND_NEEDS_BUILD" = "true" ]; then
    echo -e "${YELLOW}[4/4]${NC} 构建后端..."
    bash "$SCRIPT_DIR/build-backend-optimized.sh"
    echo ""
else
    echo -e "${YELLOW}[4/4]${NC} 跳过后端构建"
    echo ""
fi

# 构建前端（如果需要）
if [ "$FRONTEND_NEEDS_BUILD" = "true" ]; then
    echo -e "${YELLOW}[4/4]${NC} 构建前端..."
    bash "$SCRIPT_DIR/build-frontend-optimized.sh"
    echo ""
else
    echo -e "${YELLOW}[4/4]${NC} 跳过前端构建"
    echo ""
fi

# 创建发布包
echo -e "${YELLOW}[5/5]${NC} 创建发布包..."
TIMESTAMP=$(date +"%Y%m%d")
PACKAGE_NAME="starrocks-admin-$TIMESTAMP.tar.gz"
PACKAGE_PATH="$DIST_DIR/$PACKAGE_NAME"

echo "包名: $PACKAGE_NAME"
cd "$DIST_DIR"
tar -czf "$PACKAGE_NAME" --transform 's,^,starrocks-admin/,' *
echo "包已创建: $PACKAGE_PATH"
echo "解压命令: tar -xzf $PACKAGE_NAME"

# 显示构建摘要
show_build_summary "$BACKEND_NEEDS_BUILD" "$FRONTEND_NEEDS_BUILD" "$DIST_DIR"

echo -e "${GREEN}构建完成！${NC}"
