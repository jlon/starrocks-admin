#!/usr/bin/env bash

#
# StarRocks Admin - Build Utilities
# 构建工具函数库
#

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 获取项目根目录
get_project_root() {
    echo "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
}

# 检查文件是否有修改
check_files_changed() {
    local dir="$1"
    local last_build_file="$2"
    local exclude_patterns="$3"
    
    if [ ! -f "$last_build_file" ]; then
        echo "true"  # 首次构建
        return
    fi
    
    local last_build_time=$(cat "$last_build_file")
    local current_time=$(date +%s)
    
    # 检查目录是否存在
    if [ ! -d "$dir" ]; then
        echo "false"
        return
    fi
    
    # 构建 find 命令
    local find_cmd="find \"$dir\" -type f -newer \"$last_build_file\""
    
    # 添加排除模式
    if [ -n "$exclude_patterns" ]; then
        IFS=',' read -ra patterns <<< "$exclude_patterns"
        for pattern in "${patterns[@]}"; do
            find_cmd="$find_cmd ! -path \"$pattern\""
        done
    fi
    
    # 执行查找
    local changed_files=$(eval "$find_cmd" 2>/dev/null | head -1)
    
    if [ -n "$changed_files" ]; then
        echo "true"
    else
        echo "false"
    fi
}

# 检查后端是否有修改
check_backend_changed() {
    local project_root="$1"
    local backend_dir="$project_root/backend"
    local last_build_file="$project_root/build/.last_backend_build"
    local exclude_patterns="*/target/*,*/Cargo.lock"
    
    check_files_changed "$backend_dir" "$last_build_file" "$exclude_patterns"
}

# 检查前端是否有修改
check_frontend_changed() {
    local project_root="$1"
    local frontend_dir="$project_root/frontend"
    local last_build_file="$project_root/build/.last_frontend_build"
    local exclude_patterns="*/node_modules/*,*/dist/*,*/coverage/*"
    
    check_files_changed "$frontend_dir" "$last_build_file" "$exclude_patterns"
}

# 更新构建时间戳
update_build_timestamp() {
    local timestamp_file="$1"
    local timestamp_dir=$(dirname "$timestamp_file")
    
    mkdir -p "$timestamp_dir"
    date +%s > "$timestamp_file"
}

# 显示构建状态
show_build_status() {
    local component="$1"
    local needs_build="$2"
    local reason="$3"
    
    if [ "$needs_build" = "true" ]; then
        echo -e "${YELLOW}[BUILD]${NC} $component 需要构建: $reason"
    else
        echo -e "${GREEN}[SKIP]${NC} $component 无需构建: $reason"
    fi
}

# 检查构建依赖
check_build_dependencies() {
    local missing_deps=()
    
    # 检查 Rust/Cargo
    if ! command -v cargo &> /dev/null; then
        missing_deps+=("Rust/Cargo")
    fi
    
    # 检查 Node.js/npm
    if ! command -v npm &> /dev/null; then
        missing_deps+=("Node.js/npm")
    fi
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        echo -e "${RED}[ERROR]${NC} 缺少构建依赖:"
        for dep in "${missing_deps[@]}"; do
            echo -e "  - $dep"
        done
        return 1
    fi
    
    return 0
}

# 清理构建缓存
clean_build_cache() {
    local project_root="$1"
    local backend_dir="$project_root/backend"
    local frontend_dir="$project_root/frontend"
    
    echo -e "${YELLOW}[CLEAN]${NC} 清理构建缓存..."
    
    # 清理 Rust 构建缓存
    if [ -d "$backend_dir/target" ]; then
        cd "$backend_dir"
        cargo clean
        echo -e "${GREEN}✓${NC} 清理 Rust 构建缓存"
    fi
    
    # 清理 Node.js 构建缓存
    if [ -d "$frontend_dir/node_modules/.cache" ]; then
        rm -rf "$frontend_dir/node_modules/.cache"
        echo -e "${GREEN}✓${NC} 清理 Node.js 构建缓存"
    fi
    
    # 清理前端 dist 目录
    if [ -d "$frontend_dir/dist" ]; then
        rm -rf "$frontend_dir/dist"
        echo -e "${GREEN}✓${NC} 清理前端 dist 目录"
    fi
    
    # 清理 build 目录（可选，用于 make clean）
    # 注意：build-clean 不清理 build 目录，只清理编译缓存
}

# 显示构建摘要
show_build_summary() {
    local backend_built="$1"
    local frontend_built="$2"
    local dist_dir="$3"
    
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}构建摘要${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
    
    if [ "$backend_built" = "true" ]; then
        echo -e "${GREEN}✓${NC} 后端已构建"
    else
        echo -e "${YELLOW}⚠${NC} 后端跳过构建"
    fi
    
    if [ "$frontend_built" = "true" ]; then
        echo -e "${GREEN}✓${NC} 前端已构建"
    else
        echo -e "${YELLOW}⚠${NC} 前端跳过构建"
    fi
    
    echo ""
    echo -e "${BLUE}输出目录:${NC} $dist_dir"
    echo -e "${BLUE}二进制文件:${NC} $dist_dir/bin/starrocks-admin"
    echo -e "${BLUE}Web 文件:${NC} $dist_dir/web/"
    echo ""
}
