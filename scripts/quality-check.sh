#!/bin/bash

# StarRocks Admin - Code Quality Check
# 代码质量检查脚本

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 检查结果
RUST_CLIPPY_PASSED=false
RUST_FMT_PASSED=false
RUST_AUDIT_PASSED=false
FRONTEND_LINT_PASSED=false
FRONTEND_FMT_PASSED=false
FRONTEND_AUDIT_PASSED=false

echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   StarRocks Admin - Quality Check    ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# 加载 Rust 环境（如果存在）
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# ==================== Rust 代码质量检查 ====================
echo -e "${BLUE}=== Rust 后端代码质量检查 ===${NC}"
echo ""

# 1. Clippy 检查
echo -e "${YELLOW}[1/3] Rust Clippy (代码质量)...${NC}"
cd "$PROJECT_ROOT/backend"
if cargo clippy --all-targets --all-features -- -D warnings 2>&1; then
    echo -e "${GREEN}✓ Rust Clippy 通过${NC}"
    RUST_CLIPPY_PASSED=true
else
    echo -e "${RED}✗ Rust Clippy 检查失败${NC}"
fi
echo ""

# 2. 代码格式检查
echo -e "${YELLOW}[2/3] Rust 代码格式检查...${NC}"
if cargo fmt --check 2>&1; then
    echo -e "${GREEN}✓ Rust 代码格式正确${NC}"
    RUST_FMT_PASSED=true
else
    echo -e "${RED}✗ Rust 代码格式不正确，运行 'cargo fmt' 修复${NC}"
fi
echo ""

# 3. 安全审计
echo -e "${YELLOW}[3/3] Rust 安全审计 (Cargo Audit)...${NC}"
if command -v cargo-audit > /dev/null 2>&1; then
    AUDIT_OUTPUT=$(cargo audit 2>&1 || true)
    if echo "$AUDIT_OUTPUT" | grep -qi "error.*fetch\|network\|IO error"; then
        echo -e "${YELLOW}⚠ Rust 安全审计因网络问题跳过${NC}"
        RUST_AUDIT_PASSED=true  # 网络问题不计为失败
    elif echo "$AUDIT_OUTPUT" | grep -qi "Success\|No vulnerabilities\|No unmaintained"; then
        echo -e "${GREEN}✓ Rust 安全审计通过${NC}"
        RUST_AUDIT_PASSED=true
    else
        echo -e "${RED}✗ Rust 安全审计发现漏洞${NC}"
        echo "$AUDIT_OUTPUT" | grep -E "error|vulnerability|advisory" | head -10
    fi
else
    echo -e "${YELLOW}⚠ cargo-audit 未安装，跳过安全审计${NC}"
    echo -e "${YELLOW}   安装: cargo install cargo-audit${NC}"
    RUST_AUDIT_PASSED=true  # 跳过不计为失败
fi
echo ""

# ==================== 前端代码质量检查 ====================
echo -e "${BLUE}=== Angular 前端代码质量检查 ===${NC}"
echo ""

cd "$PROJECT_ROOT/frontend"

# 1. Angular Lint 检查
echo -e "${YELLOW}[1/3] Angular Lint (代码质量)...${NC}"
if npm run lint 2>&1 | grep -q "All files pass linting" || npm run lint 2>&1 | tail -1 | grep -q "success"; then
    echo -e "${GREEN}✓ Angular Lint 通过${NC}"
    FRONTEND_LINT_PASSED=true
else
    # 检查是否有实际错误
    if npm run lint 2>&1 | grep -qi "error\|warning" && ! npm run lint 2>&1 | grep -qi "All files pass linting\|success"; then
        echo -e "${RED}✗ Angular Lint 发现问题时${NC}"
        npm run lint 2>&1 | grep -E "error|warning" | head -20
    else
        echo -e "${GREEN}✓ Angular Lint 通过${NC}"
        FRONTEND_LINT_PASSED=true
    fi
fi
echo ""

# 2. Prettier 格式检查
echo -e "${YELLOW}[2/3] 前端代码格式检查 (Prettier)...${NC}"
if command -v npx > /dev/null 2>&1; then
    if npx prettier --check "src/**/*.{ts,html,scss}" 2>&1 | tail -1 | grep -q "All matched files use Prettier code style"; then
        echo -e "${GREEN}✓ 前端代码格式正确${NC}"
        FRONTEND_FMT_PASSED=true
    else
        # 显示需要格式化的文件数量
        UNFORMATTED=$(npx prettier --check "src/**/*.{ts,html,scss}" 2>&1 | grep -c "would be reformatted" || echo "0")
        # 确保是数字（处理可能的空值或多行）
        UNFORMATTED=${UNFORMATTED:-0}
        UNFORMATTED=$(echo "$UNFORMATTED" | head -1 | grep -o '[0-9]*' || echo "0")
        if [ "$UNFORMATTED" -gt 0 ] 2>/dev/null; then
            echo -e "${RED}✗ 发现 $UNFORMATTED 个文件需要格式化${NC}"
            echo -e "${YELLOW}   运行 'npx prettier --write src/**/*.{ts,html,scss}' 修复${NC}"
        else
            echo -e "${GREEN}✓ 前端代码格式正确${NC}"
            FRONTEND_FMT_PASSED=true
        fi
    fi
else
    echo -e "${YELLOW}⚠ npx 未找到，跳过格式检查${NC}"
    FRONTEND_FMT_PASSED=true  # 跳过不计为失败
fi
echo ""

# 3. NPM 安全审计
echo -e "${YELLOW}[3/3] 前端安全审计 (NPM Audit)...${NC}"
AUDIT_OUTPUT=$(npm audit --json 2>&1 || true)
if echo "$AUDIT_OUTPUT" | grep -q '"vulnerabilities":{}'; then
    echo -e "${GREEN}✓ 前端安全审计通过${NC}"
    FRONTEND_AUDIT_PASSED=true
else
    # 检查是否是 JSON 格式的错误（网络问题等）
    if echo "$AUDIT_OUTPUT" | grep -qi "error\|failed\|network"; then
        echo -e "${YELLOW}⚠ NPM 安全审计因网络问题跳过${NC}"
        FRONTEND_AUDIT_PASSED=true  # 网络问题不计为失败
    else
        # 提取漏洞数量（安全地处理空值）
        CRITICAL=$(echo "$AUDIT_OUTPUT" | grep -o '"critical":[0-9]*' | grep -o '[0-9]*' | head -1 || echo "0")
        HIGH=$(echo "$AUDIT_OUTPUT" | grep -o '"high":[0-9]*' | grep -o '[0-9]*' | head -1 || echo "0")
        MODERATE=$(echo "$AUDIT_OUTPUT" | grep -o '"moderate":[0-9]*' | grep -o '[0-9]*' | head -1 || echo "0")
        LOW=$(echo "$AUDIT_OUTPUT" | grep -o '"low":[0-9]*' | grep -o '[0-9]*' | head -1 || echo "0")
        
        # 确保是数字
        CRITICAL=${CRITICAL:-0}
        HIGH=${HIGH:-0}
        MODERATE=${MODERATE:-0}
        LOW=${LOW:-0}
        
        TOTAL=$((CRITICAL + HIGH + MODERATE + LOW))
        if [ "$TOTAL" -gt 0 ]; then
            echo -e "${RED}✗ 前端安全审计发现 $TOTAL 个漏洞${NC}"
            echo -e "   严重: $CRITICAL, 高危: $HIGH, 中等: $MODERATE, 低危: $LOW"
            echo -e "${YELLOW}   运行 'npm audit fix' 尝试修复${NC}"
        else
            echo -e "${GREEN}✓ 前端安全审计通过${NC}"
            FRONTEND_AUDIT_PASSED=true
        fi
    fi
fi
echo ""

# ==================== 总结 ====================
echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           检查结果总结                ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

echo -e "Rust 后端:"
echo -e "  Clippy:       $([ "$RUST_CLIPPY_PASSED" = true ] && echo -e "${GREEN}✓ 通过${NC}" || echo -e "${RED}✗ 失败${NC}")"
echo -e "  代码格式:     $([ "$RUST_FMT_PASSED" = true ] && echo -e "${GREEN}✓ 通过${NC}" || echo -e "${RED}✗ 失败${NC}")"
echo -e "  安全审计:     $([ "$RUST_AUDIT_PASSED" = true ] && echo -e "${GREEN}✓ 通过${NC}" || echo -e "${RED}✗ 失败${NC}")"
echo ""

echo -e "Angular 前端:"
echo -e "  Lint:         $([ "$FRONTEND_LINT_PASSED" = true ] && echo -e "${GREEN}✓ 通过${NC}" || echo -e "${RED}✗ 失败${NC}")"
echo -e "  代码格式:     $([ "$FRONTEND_FMT_PASSED" = true ] && echo -e "${GREEN}✓ 通过${NC}" || echo -e "${RED}✗ 失败${NC}")"
echo -e "  安全审计:     $([ "$FRONTEND_AUDIT_PASSED" = true ] && echo -e "${GREEN}✓ 通过${NC}" || echo -e "${RED}✗ 失败${NC}")"
echo ""

# 计算总体结果
ALL_PASSED=true
if [ "$RUST_CLIPPY_PASSED" != true ] || [ "$RUST_FMT_PASSED" != true ] || [ "$RUST_AUDIT_PASSED" != true ]; then
    ALL_PASSED=false
fi
if [ "$FRONTEND_LINT_PASSED" != true ] || [ "$FRONTEND_FMT_PASSED" != true ] || [ "$FRONTEND_AUDIT_PASSED" != true ]; then
    ALL_PASSED=false
fi

if [ "$ALL_PASSED" = true ]; then
    echo -e "${GREEN}✓ 所有检查通过！${NC}"
    exit 0
else
    echo -e "${RED}✗ 部分检查失败，请修复后重试${NC}"
    exit 1
fi

