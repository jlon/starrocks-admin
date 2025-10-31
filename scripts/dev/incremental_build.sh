#!/bin/bash

# StarRocks Admin - Incremental Build Script
# 增量构建脚本，智能判断是否需要重新编译

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/backend"

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# 检查是否需要重新编译
check_build_needed() {
    local force_build="${1:-false}"
    
    if [ "$force_build" = "true" ]; then
        echo -e "${YELLOW}强制构建模式${NC}"
        return 0
    fi
    
    cd "$BACKEND_DIR"
    
    # 检查 Cargo.toml 是否比 target 目录新
    if [ "Cargo.toml" -nt "target" ] 2>/dev/null; then
        echo -e "${YELLOW}Cargo.toml 已更新，需要重新编译${NC}"
        return 0
    fi
    
    # 检查是否有源代码文件比 target 目录新
    if find src -name "*.rs" -newer target 2>/dev/null | grep -q .; then
        echo -e "${YELLOW}源代码已更新，需要重新编译${NC}"
        return 0
    fi
    
    # 检查 target 目录是否存在且完整
    if [ ! -d "target/debug" ] || [ ! -f "target/debug/deps/libstarrocks_admin-*.rlib" ] 2>/dev/null; then
        echo -e "${YELLOW}编译缓存不完整，需要重新编译${NC}"
        return 0
    fi
    
    echo -e "${GREEN}无需重新编译，使用现有缓存${NC}"
    return 1
}

# 执行增量构建
incremental_build() {
    local force_build="${1:-false}"
    
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}StarRocks Admin - Incremental Build${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
    
    cd "$BACKEND_DIR"
    
    if check_build_needed "$force_build"; then
        echo -e "${YELLOW}[1/3]${NC} 准备编译环境..."
        
        # 设置环境变量优化编译
        export CARGO_INCREMENTAL=1
        export CARGO_TARGET_DIR="target"
        
        echo -e "${YELLOW}[2/3]${NC} 编译依赖库（如果未编译）..."
        # 先编译依赖，使用 release 模式获得更好性能
        cargo build --release --dependencies-only 2>/dev/null || true
        
        echo -e "${YELLOW}[3/3]${NC} 编译项目代码..."
        # 编译项目代码，使用 debug 模式便于调试
        cargo build
        
        echo -e "${GREEN}✓ 编译完成${NC}"
    else
        echo -e "${GREEN}✓ 使用现有编译缓存${NC}"
    fi
    
    echo ""
}

# 主函数
main() {
    local force_build="${1:-false}"
    incremental_build "$force_build"
}

# 如果直接运行此脚本
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    main "$@"
fi
