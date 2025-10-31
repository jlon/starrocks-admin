#!/bin/bash

# StarRocks Admin - Development Environment Logs Viewer
# 实时查看开发环境日志

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Colors
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}=== Development Environment Logs ===${NC}"
echo ""
echo "Press Ctrl+C to exit..."
echo ""

# Check which services are running
BACKEND_RUNNING=false
FRONTEND_RUNNING=false

if [ -f "$PROJECT_ROOT/backend/starrocks-admin.pid" ]; then
    BACKEND_RUNNING=true
fi

if [ -f "$PROJECT_ROOT/frontend/frontend.pid" ]; then
    FRONTEND_RUNNING=true
fi

if [ "$BACKEND_RUNNING" = true ] && [ "$FRONTEND_RUNNING" = true ]; then
    # Both services running - show combined logs
    echo -e "${YELLOW}Showing combined logs (backend + frontend)...${NC}"
    echo ""
    # Use tail to follow both log files
    tail -f "$PROJECT_ROOT/backend/logs/starrocks-admin.log" "$PROJECT_ROOT/frontend/frontend.log"
elif [ "$BACKEND_RUNNING" = true ]; then
    # Only backend running
    echo -e "${YELLOW}Showing backend logs...${NC}"
    echo ""
    tail -f "$PROJECT_ROOT/backend/logs/starrocks-admin.log"
elif [ "$FRONTEND_RUNNING" = true ]; then
    # Only frontend running
    echo -e "${YELLOW}Showing frontend logs...${NC}"
    echo ""
    tail -f "$PROJECT_ROOT/frontend/frontend.log"
else
    echo -e "${YELLOW}No services running.${NC}"
fi

