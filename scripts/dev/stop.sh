#!/bin/bash

# StarRocks Admin - Development Environment Stop Script
# 停止开发环境中的所有服务（复用状态检查逻辑）

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# 从共享配置读取端口（复用 check_status.sh 的配置）
SHARED_CONFIG="$PROJECT_ROOT/conf/shared.json"
if command -v jq > /dev/null 2>&1 && [ -f "$SHARED_CONFIG" ]; then
    BACKEND_PORT=$(jq -r '.dev.backend.port' "$SHARED_CONFIG" 2>/dev/null || echo "8081")
    FRONTEND_PORT=$(jq -r '.dev.frontend.port' "$SHARED_CONFIG" 2>/dev/null || echo "4200")
else
    BACKEND_PORT="8081"
    FRONTEND_PORT="4200"
fi

# 获取进程 PID（复用 check_status.sh 的逻辑）
get_process_pid() {
    local pattern=$1
    if command -v pgrep > /dev/null 2>&1; then
        pgrep -f "$pattern" | head -1
    else
        ps aux | grep -v grep | grep "$pattern" | awk '{print $2}' | head -1
    fi
}

# 停止后端服务
stop_backend() {
    local stopped=false
    
    # 方法1: 检查 PID 文件
    BACKEND_PID_FILE="$PROJECT_ROOT/backend/starrocks-admin.pid"
    if [ -f "$BACKEND_PID_FILE" ]; then
        local pid=$(cat "$BACKEND_PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo -e "${YELLOW}Stopping backend (PID: $pid from PID file)...${NC}"
            kill -TERM "$pid" 2>/dev/null || true
            sleep 2
            if ps -p "$pid" > /dev/null 2>&1; then
                kill -KILL "$pid" 2>/dev/null || true
            fi
            rm -f "$BACKEND_PID_FILE"
            stopped=true
        else
            rm -f "$BACKEND_PID_FILE"
        fi
    fi
    
    # 方法2: 通过端口查找进程
    if command -v lsof > /dev/null 2>&1; then
        local port_pid=$(lsof -ti :$BACKEND_PORT 2>/dev/null | head -1)
        if [ -n "$port_pid" ] && ps -p "$port_pid" > /dev/null 2>&1; then
            if [ "$stopped" = false ]; then
                echo -e "${YELLOW}Stopping backend (PID: $port_pid from port $BACKEND_PORT)...${NC}"
            fi
            kill -TERM "$port_pid" 2>/dev/null || true
            sleep 2
            if ps -p "$port_pid" > /dev/null 2>&1; then
                kill -KILL "$port_pid" 2>/dev/null || true
            fi
            stopped=true
        fi
    fi
    
    # 方法3: 通过进程名查找（cargo watch）
    local cargo_watch_pid=$(get_process_pid "cargo.*watch.*starrocks-admin")
    if [ -n "$cargo_watch_pid" ] && [ "$stopped" = false ]; then
        echo -e "${YELLOW}Stopping backend (cargo watch, PID: $cargo_watch_pid)...${NC}"
        kill -TERM "$cargo_watch_pid" 2>/dev/null || true
        sleep 2
        if ps -p "$cargo_watch_pid" > /dev/null 2>&1; then
            kill -KILL "$cargo_watch_pid" 2>/dev/null || true
        fi
        stopped=true
    fi
    
    # 方法4: 查找 cargo run 进程
    local cargo_run_pid=$(get_process_pid "cargo.*run.*starrocks-admin")
    if [ -n "$cargo_run_pid" ] && [ "$stopped" = false ]; then
        echo -e "${YELLOW}Stopping backend (cargo run, PID: $cargo_run_pid)...${NC}"
        kill -TERM "$cargo_run_pid" 2>/dev/null || true
        sleep 2
        if ps -p "$cargo_run_pid" > /dev/null 2>&1; then
            kill -KILL "$cargo_run_pid" 2>/dev/null || true
        fi
        stopped=true
    fi
    
    if [ "$stopped" = false ]; then
        echo -e "${GREEN}Backend not running${NC}"
        return 1
    else
        echo -e "${GREEN}Backend stopped${NC}"
        return 0
    fi
}

# 停止前端服务（复用 check_status.sh 的检测逻辑）
stop_frontend() {
    local stopped=false
    
    # 方法1: 检查 PID 文件
    FRONTEND_PID_FILE="$PROJECT_ROOT/frontend/frontend.pid"
    if [ -f "$FRONTEND_PID_FILE" ]; then
        local pid=$(cat "$FRONTEND_PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo -e "${YELLOW}Stopping frontend (PID: $pid from PID file)...${NC}"
            kill -TERM "$pid" 2>/dev/null || true
            sleep 2
            if ps -p "$pid" > /dev/null 2>&1; then
                kill -KILL "$pid" 2>/dev/null || true
            fi
            rm -f "$FRONTEND_PID_FILE"
            stopped=true
        else
            rm -f "$FRONTEND_PID_FILE"
        fi
    fi
    
    # 方法2: 在 WSL 中通过 PowerShell 检查 Windows 端口（复用 check_status.sh 的逻辑）
    if command -v powershell.exe > /dev/null 2>&1; then
        local win_pid=$(powershell.exe -Command "Get-NetTCPConnection -LocalPort ${FRONTEND_PORT} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -First 1" 2>/dev/null | tr -d '\r\n' | grep -v '^$')
        if [ -n "$win_pid" ] && [ "$win_pid" != "" ]; then
            echo -e "${YELLOW}Stopping frontend (Windows PID: $win_pid from port $FRONTEND_PORT)...${NC}"
            # 在 Windows 上停止进程
            powershell.exe -Command "Stop-Process -Id $win_pid -Force -ErrorAction SilentlyContinue" 2>/dev/null
            stopped=true
        fi
    fi
    
    # 方法3: 通过端口查找进程（WSL/Linux 环境）
    if [ "$stopped" = false ] && command -v lsof > /dev/null 2>&1; then
        local port_pid=$(lsof -ti :$FRONTEND_PORT 2>/dev/null | head -1)
        if [ -n "$port_pid" ] && ps -p "$port_pid" > /dev/null 2>&1; then
            echo -e "${YELLOW}Stopping frontend (PID: $port_pid from port $FRONTEND_PORT)...${NC}"
            kill -TERM "$port_pid" 2>/dev/null || true
            sleep 2
            if ps -p "$port_pid" > /dev/null 2>&1; then
                kill -KILL "$port_pid" 2>/dev/null || true
            fi
            stopped=true
        fi
    fi
    
    # 方法4: 通过进程名查找（ng serve）
    if [ "$stopped" = false ]; then
        local ng_serve_pid=$(get_process_pid "ng serve|@angular-devkit/build-angular:dev-server")
        if [ -n "$ng_serve_pid" ]; then
            echo -e "${YELLOW}Stopping frontend (ng serve, PID: $ng_serve_pid)...${NC}"
            kill -TERM "$ng_serve_pid" 2>/dev/null || true
            sleep 2
            if ps -p "$ng_serve_pid" > /dev/null 2>&1; then
                kill -KILL "$ng_serve_pid" 2>/dev/null || true
            fi
            stopped=true
        fi
    fi
    
    # 方法5: 查找 npm start 进程（通过检查端口和相关进程）
    if [ "$stopped" = false ]; then
        # 尝试通过 HTTP 测试端口是否还在监听（复用 check_status.sh 的逻辑）
        local http_test_result=false
        if command -v curl > /dev/null 2>&1; then
            if curl -s --connect-timeout 2 --max-time 3 "http://localhost:${FRONTEND_PORT}" > /dev/null 2>&1; then
                http_test_result=true
            elif curl -s --connect-timeout 2 --max-time 3 "http://127.0.0.1:${FRONTEND_PORT}" > /dev/null 2>&1; then
                http_test_result=true
            fi
        fi
        
        # 如果 HTTP 测试成功，尝试通过 PowerShell 获取 Windows 进程
        if [ "$http_test_result" = true ] && command -v powershell.exe > /dev/null 2>&1; then
            local win_pid=$(powershell.exe -Command "Get-NetTCPConnection -LocalPort ${FRONTEND_PORT} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -First 1" 2>/dev/null | tr -d '\r\n')
            if [ -n "$win_pid" ] && [ "$win_pid" != "" ]; then
                echo -e "${YELLOW}Stopping frontend (Windows PID: $win_pid via HTTP detection)...${NC}"
                powershell.exe -Command "Stop-Process -Id $win_pid -Force -ErrorAction SilentlyContinue" 2>/dev/null
                stopped=true
            fi
        fi
    fi
    
    if [ "$stopped" = false ]; then
        echo -e "${GREEN}Frontend not running${NC}"
        return 1
    else
        echo -e "${GREEN}Frontend stopped${NC}"
        return 0
    fi
}

# 主函数
main() {
    echo -e "${YELLOW}Stopping development environment...${NC}"
    echo ""
    
    # 停止后端
    stop_backend
    local backend_result=$?
    echo ""
    
    # 停止前端
    stop_frontend
    local frontend_result=$?
    echo ""
    
    # 即使有服务未运行，也不算错误（因为可能是正常的）
    echo -e "${GREEN}All services stopped!${NC}"
    
    # 只有当所有服务都不存在时才返回错误（避免误报）
    if [ $backend_result -ne 0 ] && [ $frontend_result -ne 0 ]; then
        return 0
    fi
    return 0
}

# 执行主函数（不设置 set -e，允许某些服务未运行）
set +e
main "$@"