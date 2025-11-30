#!/bin/bash

# StarRocks Admin Frontend Cleanup Script
# 该脚本用于清理未使用的代码和依赖
# 使用前请确保已备份代码: git checkout -b feature/frontend-cleanup

set -e  # Exit on error

echo "======================================"
echo "StarRocks Admin Frontend Cleanup"
echo "======================================"
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查是否在 frontend 目录
if [ ! -f "package.json" ]; then
    echo -e "${RED}错误: 请在 frontend 目录下运行此脚本${NC}"
    exit 1
fi

# 询问确认
echo -e "${YELLOW}此脚本将删除未使用的代码和依赖，请确认:${NC}"
echo "1. 已创建 Git 备份分支"
echo "2. 已全面测试当前功能"
echo ""
read -p "是否继续? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "取消操作"
    exit 1
fi

echo ""
echo "======================================"
echo "步骤 1: 删除未使用的依赖包"
echo "======================================"

PACKAGES_TO_REMOVE=(
    "ckeditor"
    "ng2-ckeditor"
    "@asymmetrik/ngx-leaflet"
    "leaflet"
    "@swimlane/ngx-charts"
    "angular2-chartjs"
    "chart.js"
    "ng2-completer"
    "countup.js"
    "pace-js"
    "ionicons"
    "socicon"
    "typeface-exo"
    "tinymce"
)

echo "将删除以下依赖包:"
for pkg in "${PACKAGES_TO_REMOVE[@]}"; do
    echo "  - $pkg"
done
echo ""

read -p "开始删除依赖? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "删除依赖包..."
    npm uninstall "${PACKAGES_TO_REMOVE[@]}"
    echo -e "${GREEN}✓ 依赖包删除完成${NC}"
else
    echo -e "${YELLOW}跳过依赖包删除${NC}"
fi

echo ""
echo "======================================"
echo "步骤 2: 删除 Mock 数据服务"
echo "======================================"

if [ -d "src/app/@core/mock" ]; then
    echo "将删除: src/app/@core/mock/ (21个文件)"
    read -p "确认删除? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf src/app/@core/mock/
        echo -e "${GREEN}✓ Mock 数据服务已删除${NC}"
    else
        echo -e "${YELLOW}跳过 Mock 数据服务删除${NC}"
    fi
else
    echo -e "${YELLOW}Mock 目录不存在，跳过${NC}"
fi

echo ""
echo "======================================"
echo "步骤 3: 删除未使用的数据接口"
echo "======================================"

DATA_FILES=(
    "src/app/@core/data/country-order.ts"
    "src/app/@core/data/earning.ts"
    "src/app/@core/data/electricity.ts"
    "src/app/@core/data/orders-chart.ts"
    "src/app/@core/data/orders-profit-chart.ts"
    "src/app/@core/data/profit-bar-animation-chart.ts"
    "src/app/@core/data/profit-chart.ts"
    "src/app/@core/data/security-cameras.ts"
    "src/app/@core/data/solar.ts"
    "src/app/@core/data/stats-bar.ts"
    "src/app/@core/data/stats-progress-bar.ts"
    "src/app/@core/data/temperature-humidity.ts"
    "src/app/@core/data/traffic-bar.ts"
    "src/app/@core/data/traffic-chart.ts"
    "src/app/@core/data/traffic-list.ts"
    "src/app/@core/data/user-activity.ts"
    "src/app/@core/data/visitors-analytics.ts"
)

echo "将删除 ${#DATA_FILES[@]} 个未使用的数据接口文件"
read -p "确认删除? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    for file in "${DATA_FILES[@]}"; do
        if [ -f "$file" ]; then
            rm "$file"
            echo "  删除: $file"
        fi
    done
    echo -e "${GREEN}✓ 数据接口文件已删除${NC}"
else
    echo -e "${YELLOW}跳过数据接口文件删除${NC}"
fi

echo ""
echo "======================================"
echo "步骤 4: 删除 TinyMCE 资源"
echo "======================================"

if [ -d "src/assets/skins/lightgray" ]; then
    echo "将删除: src/assets/skins/lightgray/"
    read -p "确认删除? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf src/assets/skins/lightgray/
        echo -e "${GREEN}✓ TinyMCE 资源已删除${NC}"
    else
        echo -e "${YELLOW}跳过 TinyMCE 资源删除${NC}"
    fi
else
    echo -e "${YELLOW}TinyMCE 资源目录不存在，跳过${NC}"
fi

echo ""
echo "======================================"
echo "步骤 5: 清理未使用的主题 (可选)"
echo "======================================"

THEME_FILES=(
    "src/app/@theme/styles/theme.cosmic.ts"
    "src/app/@theme/styles/theme.corporate.ts"
    "src/app/@theme/styles/theme.dark.ts"
)

echo "将删除 ${#THEME_FILES[@]} 个未使用的主题文件"
read -p "确认删除? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    for file in "${THEME_FILES[@]}"; do
        if [ -f "$file" ]; then
            rm "$file"
            echo "  删除: $file"
        fi
    done
    echo -e "${GREEN}✓ 主题文件已删除${NC}"
    echo -e "${YELLOW}注意: 需要手动更新 theme.module.ts 删除主题导入${NC}"
else
    echo -e "${YELLOW}跳过主题文件删除${NC}"
fi

echo ""
echo "======================================"
echo "步骤 6: 删除未使用的布局 (可选)"
echo "======================================"

LAYOUT_DIRS=(
    "src/app/@theme/layouts/two-columns"
    "src/app/@theme/layouts/three-columns"
)

echo "将删除 ${#LAYOUT_DIRS[@]} 个未使用的布局"
read -p "确认删除? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    for dir in "${LAYOUT_DIRS[@]}"; do
        if [ -d "$dir" ]; then
            rm -rf "$dir"
            echo "  删除: $dir"
        fi
    done
    echo -e "${GREEN}✓ 布局目录已删除${NC}"
    echo -e "${YELLOW}注意: 需要手动更新 theme.module.ts 删除布局导入${NC}"
else
    echo -e "${YELLOW}跳过布局目录删除${NC}"
fi

echo ""
echo "======================================"
echo "清理完成统计"
echo "======================================"

if [ -d "node_modules" ]; then
    NODE_MODULES_SIZE=$(du -sh node_modules | cut -f1)
    echo "node_modules 当前大小: $NODE_MODULES_SIZE"
fi

echo ""
echo -e "${GREEN}清理脚本执行完成！${NC}"
echo ""
echo "下一步操作:"
echo "1. 运行 'npm install' 重新安装依赖"
echo "2. 手动更新 src/app/@core/core.module.ts (删除 mock 服务导入)"
echo "3. 如删除了主题，更新 src/app/@theme/theme.module.ts"
echo "4. 运行 'npm start' 测试开发环境"
echo "5. 运行 'npm run build:prod' 测试生产构建"
echo "6. 全面测试应用功能"
echo ""
echo "参考文档: FRONTEND_OPTIMIZATION_REPORT.md"
echo ""
