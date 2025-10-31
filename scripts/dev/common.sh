#!/bin/bash

# StarRocks Admin - Common Development Functions
# 开发环境公共函数库

# 颜色定义
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# 检测是否在 WSL 环境中
# 返回值: 0=是WSL, 1=不是WSL
detect_wsl() {
    # 方法1: 检查 /proc/version
    if [ -f /proc/version ] && grep -qi microsoft /proc/version 2>/dev/null; then
        return 0
    fi
    
    # 方法2: 检查 uname -r
    if command -v uname > /dev/null 2>&1 && uname -r | grep -qi microsoft 2>/dev/null; then
        return 0
    fi
    
    return 1
}

# 显示 WSL 环境检测结果
print_wsl_status() {
    if detect_wsl; then
        echo -e "${GREEN}✓ 检测到 WSL 环境${NC}"
        return 0
    else
        echo -e "${YELLOW}⚠ 无法确认是否在 WSL 环境中${NC}"
        return 1
    fi
}

# 获取 WSL 默认发行版名称
# 注意: 此函数应该在 WSL 环境中运行，或者在 Windows PowerShell/CMD 中运行
get_wsl_default_distro() {
    # 如果在 WSL 中运行，尝试获取当前发行版名称
    if detect_wsl; then
        # WSL2: 从 /etc/os-release 获取
        if [ -f /etc/os-release ]; then
            . /etc/os-release
            echo "$ID"
            return 0
        fi
    fi
    
    # 如果在 Windows 中运行，尝试通过 wsl.exe 获取默认发行版
    if command -v wsl.exe > /dev/null 2>&1; then
        # 获取默认发行版（标记为 * 的）
        local distro=$(wsl.exe --list --verbose 2>/dev/null | grep '^\*' | awk '{print $NF}' | head -1)
        if [ -n "$distro" ]; then
            echo "$distro"
            return 0
        fi
    fi
    
    # 如果在 WSL 中运行，尝试从 PowerShell 获取
    if detect_wsl && command -v powershell.exe > /dev/null 2>&1; then
        local distro=$(powershell.exe -Command "wsl --list --verbose | Select-String '\*' | ForEach-Object { \$_.Line.Split([char[]]@(' '), [StringSplitOptions]::RemoveEmptyEntries)[-1] }" 2>/dev/null | tr -d '\r\n' | head -1)
        if [ -n "$distro" ]; then
            echo "$distro"
            return 0
        fi
    fi
    
    # 如果无法获取，返回空字符串（将使用 WSL 默认发行版）
    echo ""
    return 1
}

# 验证工具链是否在 WSL 中（不是 Windows 版本）
# 参数: $1 = 工具名称 (cargo, node, npm, etc.)
# 参数: $2 = 工具显示名称 (Cargo, Node.js, npm, etc.)
# 返回值: 0=有效, 1=无效
verify_wsl_tool() {
    local tool_name="$1"
    local tool_display="${2:-$1}"
    
    # 检查工具是否安装
    if ! command -v "$tool_name" > /dev/null 2>&1; then
        echo -e "${RED}✗ $tool_display 未在 WSL 中安装${NC}"
        return 1
    fi
    
    local tool_path=$(which "$tool_name")
    echo -e "${GREEN}✓ $tool_display 路径: $tool_path${NC}"
    
    # 检查是否使用 Windows 版本的工具
    if [[ "$tool_path" == *"/mnt/"* ]] || [[ "$tool_path" == *"Program Files"* ]] || [[ "$tool_path" == *"Windows"* ]]; then
        echo -e "${RED}✗ 警告：检测到可能使用 Windows 版本的 $tool_display${NC}"
        echo -e "${RED}✗ 路径: $tool_path${NC}"
        echo -e "${YELLOW}必须在 WSL 中运行，请确保 $tool_display 已安装在 WSL 中${NC}"
        return 1
    else
        echo -e "${GREEN}✓ 使用 WSL 版本的 $tool_display${NC}"
        return 0
    fi
}

# 构建 WSL wsl 命令（如果需要指定发行版）
# 返回: wsl 命令字符串，如果不需要指定发行版则返回空字符串
build_wsl_command() {
    local distro=$(get_wsl_default_distro 2>/dev/null)
    if [ -n "$distro" ]; then
        echo "wsl -d $distro"
    else
        echo "wsl"
    fi
}

