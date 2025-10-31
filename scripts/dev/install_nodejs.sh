#!/bin/bash

# StarRocks Admin - Node.js 安装脚本（WSL）
# 在 WSL 中安装 Node.js

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}════════════════════════════════════════${NC}"
echo -e "${BLUE}Node.js 安装助手 (WSL)${NC}"
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo ""

# 检查是否已经有 Node.js
if command -v node >/dev/null 2>&1; then
    local node_version=$(node --version)
    echo -e "${GREEN}✓ Node.js 已安装: $node_version${NC}"
    echo -e "${GREEN}✓ Node.js 路径: $(which node)${NC}"
    
    # 检查是否是 WSL 版本的 Node.js
    local node_path=$(which node)
    if [[ "$node_path" == *"mnt"* ]] || [[ "$node_path" == *"Windows"* ]]; then
        echo -e "${YELLOW}⚠ 检测到可能使用 Windows 版本的 Node.js${NC}"
        echo -e "${YELLOW}建议在 WSL 中安装 Node.js 以获得更好的兼容性${NC}"
    else
        echo -e "${GREEN}✓ 使用的是 WSL 版本的 Node.js${NC}"
        exit 0
    fi
fi

echo -e "${YELLOW}Node.js 未在 WSL 中安装或需要更新${NC}"
echo ""
echo -e "将安装 Node.js LTS 版本到 WSL 环境"
echo -e "安装步骤："
echo -e "  1. 添加 NodeSource 仓库"
echo -e "  2. 安装 Node.js LTS 版本"
echo ""

# 检查是否在交互式终端中
if [ -t 0 ]; then
    read -p "是否继续安装？(y/N): " -n 1 -r
    echo ""
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${YELLOW}已取消安装${NC}"
        echo ""
        echo -e "您可以稍后手动安装："
        echo -e "  curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -"
        echo -e "  sudo apt-get install -y nodejs"
        exit 0
    fi
else
    # 非交互式模式，直接安装或提示
    echo -e "${YELLOW}非交互式模式，将自动安装${NC}"
fi

echo ""
echo -e "${YELLOW}[1/2]${NC} 添加 NodeSource 仓库..."
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -

echo ""
echo -e "${YELLOW}[2/2]${NC} 安装 Node.js..."
sudo apt-get install -y nodejs

echo ""
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo -e "${GREEN}Node.js 安装完成！${NC}"
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo ""

# 验证安装
if command -v node >/dev/null 2>&1; then
    local node_version=$(node --version)
    local npm_version=$(npm --version)
    echo -e "${GREEN}✓ Node.js 版本: $node_version${NC}"
    echo -e "${GREEN}✓ npm 版本: $npm_version${NC}"
    echo -e "${GREEN}✓ Node.js 路径: $(which node)${NC}"
else
    echo -e "${RED}✗ 安装可能失败，请检查错误信息${NC}"
    exit 1
fi

echo ""

