#!/bin/bash

# StarRocks Admin - Unified Service Status Checker
# 统一的服务状态检查脚本（支持多种启动方式）

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

# 从共享配置读取端口
SHARED_CONFIG="$PROJECT_ROOT/conf/shared.json"
if command -v jq > /dev/null 2>&1 && [ -f "$SHARED_CONFIG" ]; then
    BACKEND_PORT=$(jq -r '.dev.backend.port' "$SHARED_CONFIG" 2>/dev/null || echo "8081")
    FRONTEND_PORT=$(jq -r '.dev.frontend.port' "$SHARED_CONFIG" 2>/dev/null || echo "4200")
else
    BACKEND_PORT="8081"
    FRONTEND_PORT="4200"
fi

# 检查端口是否被监听
check_port() {
    local port=$1
    # 先尝试直接连接测试（最可靠）
    if timeout 1 bash -c "echo > /dev/tcp/localhost/$port" 2>/dev/null; then
        return 0
    fi
    # 使用工具检查端口
    if command -v lsof > /dev/null 2>&1; then
        lsof -i :$port > /dev/null 2>&1 && return 0
    fi
    if command -v netstat > /dev/null 2>&1; then
        netstat -tuln 2>/dev/null | grep -q ":$port " && return 0
    fi
    if command -v ss > /dev/null 2>&1; then
        ss -tuln 2>/dev/null | grep -q ":$port " && return 0
    fi
    # 使用 curl 测试端口（最后手段）
    if command -v curl > /dev/null 2>&1; then
        curl -s --connect-timeout 1 "http://localhost:$port" > /dev/null 2>&1 && return 0
    fi
    return 1
}

# 检查进程是否运行
check_process() {
    local pattern=$1
    if command -v pgrep > /dev/null 2>&1; then
        pgrep -f "$pattern" > /dev/null 2>&1
    else
        ps aux | grep -v grep | grep "$pattern" > /dev/null 2>&1
    fi
}

# 获取进程 PID
get_process_pid() {
    local pattern=$1
    if command -v pgrep > /dev/null 2>&1; then
        pgrep -f "$pattern" | head -1
    else
        ps aux | grep -v grep | grep "$pattern" | awk '{print $2}' | head -1
    fi
}

# 检查后端服务状态
check_backend_status() {
    local status_found=false
    
    # 方法1: 检查 PID 文件
    BACKEND_PID_FILE="$PROJECT_ROOT/backend/starrocks-admin.pid"
    if [ -f "$BACKEND_PID_FILE" ]; then
        local pid=$(cat "$BACKEND_PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (PID: $pid, PID文件)"
            status_found=true
        else
            rm -f "$BACKEND_PID_FILE"
        fi
    fi
    
    # 方法2: 通过 HTTP 请求检测后端服务（最可靠的方法，跨平台）
    if command -v curl > /dev/null 2>&1; then
        local http_code=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 2 --max-time 3 "http://localhost:${BACKEND_PORT}/health" 2>/dev/null || echo "000")
        if [ "$http_code" = "200" ] || [ "$http_code" = "404" ] || [ "$http_code" = "401" ]; then
            # HTTP 请求成功（200/404/401都说明服务在运行），服务正在运行
            local pid=$(get_process_pid "starrocks-admin|rust|backend.*$BACKEND_PORT|$BACKEND_PORT")
            if [ -n "$pid" ]; then
                if [ "$status_found" = false ]; then
                    echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (PID: $pid, 端口: $BACKEND_PORT)"
                    status_found=true
                fi
            else
                # 尝试通过端口查找进程
                if command -v lsof > /dev/null 2>&1; then
                    local port_pid=$(lsof -ti :$BACKEND_PORT 2>/dev/null | head -1)
                    if [ -n "$port_pid" ]; then
                        if [ "$status_found" = false ]; then
                            echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (PID: $port_pid, 端口: $BACKEND_PORT)"
                            status_found=true
                        fi
                    fi
                fi
                # 即使找不到 PID，HTTP 请求成功说明服务在运行
                if [ "$status_found" = false ]; then
                    echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (端口: $BACKEND_PORT 可访问, HTTP: $http_code)"
                    status_found=true
                fi
            fi
        fi
    fi
    
    # 方法2b: 检查端口是否被监听（备用方法）
    if [ "$status_found" = false ] && check_port "$BACKEND_PORT"; then
        local pid=$(get_process_pid "starrocks-admin|rust|backend.*$BACKEND_PORT|$BACKEND_PORT")
        if [ -n "$pid" ]; then
            echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (PID: $pid, 端口: $BACKEND_PORT)"
            status_found=true
        else
            # 尝试通过端口查找进程
            if command -v lsof > /dev/null 2>&1; then
                local port_pid=$(lsof -ti :$BACKEND_PORT 2>/dev/null | head -1)
                if [ -n "$port_pid" ] && ps -p "$port_pid" > /dev/null 2>&1; then
                    echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (PID: $port_pid, 端口: $BACKEND_PORT)"
                    status_found=true
                fi
            fi
            # 如果端口在监听，即使找不到 PID，也认为服务在运行
            if [ "$status_found" = false ]; then
                echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (端口: $BACKEND_PORT 已监听)"
                status_found=true
            fi
        fi
    fi
    
    # 方法3: 检查 cargo watch 进程
    if check_process "cargo.*watch.*starrocks-admin"; then
        local pid=$(get_process_pid "cargo.*watch.*starrocks-admin")
        if [ -n "$pid" ]; then
            if [ "$status_found" = false ]; then
                echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (热重载模式, PID: $pid)"
                status_found=true
            fi
        fi
    fi
    
    # 方法4: 检查 cargo run 进程
    if check_process "cargo.*run.*starrocks-admin"; then
        local pid=$(get_process_pid "cargo.*run.*starrocks-admin")
        if [ -n "$pid" ]; then
            if [ "$status_found" = false ]; then
                echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (PID: $pid)"
                status_found=true
            fi
        fi
    fi
    
    # 方法5: 检查是否有 Rust 进程监听端口
    if check_process "target/.*starrocks-admin"; then
        local pid=$(get_process_pid "target/.*starrocks-admin")
        if [ -n "$pid" ] && check_port "$BACKEND_PORT"; then
            if [ "$status_found" = false ]; then
                echo -e "${GREEN}[STATUS]${NC} 后端服务正在运行 (PID: $pid)"
                status_found=true
            fi
        fi
    fi
    
    # 如果找不到运行状态
    if [ "$status_found" = false ]; then
        echo -e "${RED}[STATUS]${NC} 后端服务未运行"
        return 0  # 服务未运行不算错误，只是状态信息
    fi
    
    # 显示详细信息
    echo "  - 端口: $BACKEND_PORT"
    echo "  - 健康检查: http://localhost:${BACKEND_PORT}/health"
    
    # 测试健康检查
    if curl -s "http://localhost:${BACKEND_PORT}/health" > /dev/null 2>&1; then
        echo -e "  - 健康状态: ${GREEN}✅ 正常${NC}"
    else
        echo -e "  - 健康状态: ${YELLOW}⚠️  端口已监听但健康检查失败${NC}"
    fi
    
    return 0
}

# 检查前端服务状态
check_frontend_status() {
    local status_found=false
    
    # 方法1: 检查 PID 文件
    FRONTEND_PID_FILE="$PROJECT_ROOT/frontend/frontend.pid"
    if [ -f "$FRONTEND_PID_FILE" ]; then
        local pid=$(cat "$FRONTEND_PID_FILE")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (PID: $pid, PID文件)"
            status_found=true
        else
            rm -f "$FRONTEND_PID_FILE"
        fi
    fi
    
    # 方法2: 通过 HTTP 请求检测前端服务（最可靠的方法，跨平台）
    if command -v curl > /dev/null 2>&1; then
        # 尝试多种访问方式（支持 WSL 访问 Windows 服务）
        local http_test_result=false
        
        # 方式1: localhost (WSL 本地或 Windows 本地)
        if curl -s --connect-timeout 2 --max-time 3 "http://localhost:${FRONTEND_PORT}" > /dev/null 2>&1; then
            http_test_result=true
        fi
        
        # 方式2: 127.0.0.1 (明确指定 IPv4)
        if [ "$http_test_result" = false ] && curl -s --connect-timeout 2 --max-time 3 "http://127.0.0.1:${FRONTEND_PORT}" > /dev/null 2>&1; then
            http_test_result=true
        fi
        
        # 方式3: 在 WSL 中通过 Windows hostname 访问（如果前端在 Windows 上运行）
        if [ "$http_test_result" = false ]; then
            # 在 WSL 中，Windows 主机通常是 resolv.conf 中的 nameserver
            local resolv_conf="/etc/resolv.conf"
            if [ -f "$resolv_conf" ]; then
                local windows_host=$(grep "^nameserver" "$resolv_conf" 2>/dev/null | head -1 | awk '{print $2}' 2>/dev/null)
                if [ -n "$windows_host" ] && [ "$windows_host" != "127.0.0.1" ] && [ "$windows_host" != "::1" ]; then
                    if curl -s --connect-timeout 2 --max-time 3 "http://${windows_host}:${FRONTEND_PORT}" > /dev/null 2>&1; then
                        http_test_result=true
                    fi
                fi
            fi
        fi
        
        # 方式4: 尝试通过 PowerShell 检查 Windows 端口（如果可用）
        if [ "$http_test_result" = false ] && command -v powershell.exe > /dev/null 2>&1; then
            local win_port_check=$(powershell.exe -Command "Get-NetTCPConnection -LocalPort ${FRONTEND_PORT} -ErrorAction SilentlyContinue | Select-Object -First 1" 2>/dev/null | grep -q "4200" && echo "yes" || echo "no")
            if [ "$win_port_check" = "yes" ]; then
                http_test_result=true
            fi
        fi
        
        # 如果任何方式测试成功
        if [ "$http_test_result" = true ]; then
            # HTTP 请求成功，服务正在运行
            local pid=$(get_process_pid "ng serve|node.*serve|angular.*$FRONTEND_PORT|$FRONTEND_PORT")
            if [ -n "$pid" ]; then
                if [ "$status_found" = false ]; then
                    echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (PID: $pid, 端口: $FRONTEND_PORT)"
                    status_found=true
                fi
            else
                # 尝试通过端口查找进程
                if command -v lsof > /dev/null 2>&1; then
                    local port_pid=$(lsof -ti :$FRONTEND_PORT 2>/dev/null | head -1)
                    if [ -n "$port_pid" ]; then
                        if [ "$status_found" = false ]; then
                            echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (PID: $port_pid, 端口: $FRONTEND_PORT)"
                            status_found=true
                        fi
                    fi
                fi
                # 即使找不到 PID，HTTP 请求成功说明服务在运行
                if [ "$status_found" = false ]; then
                    echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (端口: $FRONTEND_PORT 可访问)"
                    status_found=true
                fi
            fi
        fi
    fi
    
    # 方法2b: 在 WSL 中通过 PowerShell 检查 Windows 端口（跨平台检测）
    if [ "$status_found" = false ] && command -v powershell.exe > /dev/null 2>&1; then
        local win_port_result=$(powershell.exe -Command "try { \$null = Get-NetTCPConnection -LocalPort ${FRONTEND_PORT} -State Listen -ErrorAction Stop; Write-Output 'yes' } catch { Write-Output 'no' }" 2>/dev/null)
        if [ "$win_port_result" = "yes" ]; then
            # Windows 端口在监听
            local pid=""
            # 尝试获取进程 ID
            local win_pid=$(powershell.exe -Command "Get-NetTCPConnection -LocalPort ${FRONTEND_PORT} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -First 1" 2>/dev/null | tr -d '\r\n')
            if [ -n "$win_pid" ] && [ "$win_pid" != "" ]; then
                echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (Windows PID: $win_pid, 端口: $FRONTEND_PORT)"
            else
                echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (端口: $FRONTEND_PORT 在 Windows 上监听)"
            fi
            status_found=true
        fi
    fi
    
    # 方法2c: 检查端口是否被监听（备用方法）
    if [ "$status_found" = false ] && check_port "$FRONTEND_PORT"; then
        local pid=$(get_process_pid "ng serve|node.*serve|angular.*$FRONTEND_PORT|$FRONTEND_PORT")
        if [ -n "$pid" ]; then
            echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (PID: $pid, 端口: $FRONTEND_PORT)"
            status_found=true
        else
            # 尝试通过端口查找进程
            if command -v lsof > /dev/null 2>&1; then
                local port_pid=$(lsof -ti :$FRONTEND_PORT 2>/dev/null | head -1)
                if [ -n "$port_pid" ] && ps -p "$port_pid" > /dev/null 2>&1; then
                    echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (PID: $port_pid, 端口: $FRONTEND_PORT)"
                    status_found=true
                fi
            fi
            # 如果端口在监听，即使找不到 PID，也认为服务在运行
            if [ "$status_found" = false ]; then
                echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (端口: $FRONTEND_PORT 已监听)"
                status_found=true
            fi
        fi
    fi
    
    # 方法3: 检查 ng serve 进程（多种模式）
    if check_process "ng serve"; then
        local pid=$(get_process_pid "ng serve")
        if [ -n "$pid" ]; then
            if [ "$status_found" = false ]; then
                echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (ng serve, PID: $pid)"
                status_found=true
            fi
        fi
    fi
    
    # 方法4: 检查 npm/ng 进程
    if check_process "@angular-devkit/build-angular:dev-server|@angular/cli.*serve"; then
        local pid=$(get_process_pid "@angular-devkit/build-angular:dev-server|@angular/cli.*serve")
        if [ -n "$pid" ]; then
            if [ "$status_found" = false ]; then
                echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (Angular CLI, PID: $pid)"
                status_found=true
            fi
        fi
    fi
    
    # 方法5: 检查 node 进程中的 angular 相关进程
    if check_process "node.*angular|node.*serve|webpack.*serve"; then
        local pid=$(get_process_pid "node.*angular|node.*serve|webpack.*serve")
        if [ -n "$pid" ] && [ -d "/proc/$pid" ]; then
            # 检查进程是否真的在监听端口
            if check_port "$FRONTEND_PORT"; then
                if [ "$status_found" = false ]; then
                    echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (Node.js, PID: $pid)"
                    status_found=true
                fi
            fi
        fi
    fi
    
    # 方法6: 检查 npm start 进程
    if check_process "npm.*start"; then
        # 检查该进程是否在 frontend 目录下运行
        local pid=$(get_process_pid "npm.*start")
        if [ -n "$pid" ]; then
            # 简单检查：如果端口在监听，可能是前端服务
            if check_port "$FRONTEND_PORT"; then
                if [ "$status_found" = false ]; then
                    echo -e "${GREEN}[STATUS]${NC} 前端服务正在运行 (npm start, PID: $pid)"
                    status_found=true
                fi
            fi
        fi
    fi
    
    # 如果找不到运行状态
    if [ "$status_found" = false ]; then
        echo -e "${RED}[STATUS]${NC} 前端服务未运行"
        return 0  # 服务未运行不算错误，只是状态信息
    fi
    
    # 显示详细信息
    echo "  - 端口: $FRONTEND_PORT"
    echo "  - 访问地址: http://localhost:${FRONTEND_PORT}"
    
    # 测试服务是否可访问（尝试多种方式）
    local accessible=false
    if curl -s --connect-timeout 2 --max-time 3 "http://localhost:${FRONTEND_PORT}" > /dev/null 2>&1; then
        accessible=true
    elif curl -s --connect-timeout 2 --max-time 3 "http://127.0.0.1:${FRONTEND_PORT}" > /dev/null 2>&1; then
        accessible=true
    fi
    
    if [ "$accessible" = true ]; then
        echo -e "  - 健康状态: ${GREEN}✅ 可访问${NC}"
    else
        # 如果检测到服务运行但无法通过 HTTP 访问，可能是跨平台问题（WSL <-> Windows）
        echo -e "  - 健康状态: ${YELLOW}⚠️  端口已监听（可能在 Windows 上运行，从 WSL 无法直接访问）${NC}"
    fi
    
    return 0
}

# 主函数
main() {
    case "${1:-all}" in
        backend)
            check_backend_status
            ;;
        frontend)
            check_frontend_status
            ;;
        all|*)
            echo "=== Backend Status ==="
            check_backend_status
            echo ""
            echo "=== Frontend Status ==="
            check_frontend_status
            ;;
    esac
}

# 执行主函数
main "$@"
