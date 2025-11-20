# MCP Feedback Enhanced 配置指南

## 问题说明

MCP mcp-feedback-enhanced 工具在远程环境中无法自动打开浏览器，导致超时。

## 解决方案

### 1. 已完成的修复

#### ✅ 修改浏览器打开逻辑
- **文件**: `/home/oppo/.local/share/pipx/venvs/mcp-feedback-enhanced/lib/python3.12/site-packages/mcp_feedback_enhanced/web/utils/browser.py`
- **备份**: `browser.py.backup`
- **修改内容**: 在远程环境中，工具会打印 URL 供用户手动访问，而不是尝试自动打开浏览器

#### ✅ 配置固定 IP 和端口
- **IP**: `10.119.43.216`
- **端口**: `8767`
- **环境变量**: 已添加到 `~/.zshrc`
  ```bash
  export MCP_WEB_HOST="10.119.43.216"
  export MCP_WEB_PORT="8767"
  ```

### 2. 使用方法

#### 方法 A：重启 MCP 服务器（推荐）

1. **重新加载环境变量**：
   ```bash
   source ~/.zshrc
   ```

2. **重启 MCP 服务器**：
   - 关闭当前的 MCP 服务器进程
   - 重新启动 MCP 服务器

3. **使用工具时**：
   - 工具会在控制台打印访问地址
   - 手动在浏览器中打开该地址
   - 例如：`http://10.119.43.216:8767`

#### 方法 B：手动访问（临时方案）

如果工具已经启动并等待反馈：

1. **查看当前运行的端口**：
   ```bash
   netstat -tlnp 2>/dev/null | grep python
   ```

2. **在浏览器中访问**：
   - 如果端口是 8765：`http://10.119.43.216:8765`
   - 如果端口是 8766：`http://10.119.43.216:8766`
   - 如果端口是 8767：`http://10.119.43.216:8767`

### 3. 验证配置

运行以下命令验证环境变量：

```bash
echo "MCP_WEB_HOST: $MCP_WEB_HOST"
echo "MCP_WEB_PORT: $MCP_WEB_PORT"
```

预期输出：
```
MCP_WEB_HOST: 10.119.43.216
MCP_WEB_PORT: 8767
```

### 4. 测试工具

创建测试脚本：

```bash
cat > /tmp/test_mcp_feedback_fixed.py << 'SCRIPT'
#!/usr/bin/env python3
import asyncio
import sys
import os

# 设置环境变量
os.environ['MCP_WEB_HOST'] = '10.119.43.216'
os.environ['MCP_WEB_PORT'] = '8767'

async def test_feedback():
    try:
        sys.path.insert(0, '/home/oppo/.local/share/pipx/venvs/mcp-feedback-enhanced/lib/python3.12/site-packages')
        from mcp_feedback_enhanced.web import launch_web_feedback_ui
        
        print("="*80)
        print("🚀 启动 MCP Feedback 测试")
        print(f"   Web UI 地址: http://10.119.43.216:8767")
        print(f"   超时时间: 120 秒")
        print("="*80)
        
        result = await launch_web_feedback_ui(
            project_directory="/home/oppo/Documents/starrocks-admin",
            summary="测试修复后的 MCP feedback 工具\n\n请在浏览器中访问显示的地址并提交反馈",
            timeout=120
        )
        
        print("\n✅ 成功收到反馈:")
        print(f"   反馈内容: {result.get('interactive_feedback', '无')}")
        return True
        
    except TimeoutError:
        print("\n⏱️ 超时: 120秒内未收到反馈")
        print("   请确保已在浏览器中访问 http://10.119.43.216:8767")
        return False
    except Exception as e:
        print(f"\n❌ 错误: {e}")
        import traceback
        traceback.print_exc()
        return False

if __name__ == "__main__":
    success = asyncio.run(test_feedback())
    sys.exit(0 if success else 1)
SCRIPT

python3 /tmp/test_mcp_feedback_fixed.py
```

### 5. 故障排除

#### 问题：端口被占用

```bash
# 查看占用端口的进程
netstat -tlnp 2>/dev/null | grep 8767

# 或使用 ss
ss -tlnp | grep 8767

# 杀死占用进程（如果需要）
kill -9 <PID>
```

#### 问题：无法访问 Web UI

1. **检查防火墙**：
   ```bash
   sudo ufw status
   # 如果需要，允许端口
   sudo ufw allow 8767/tcp
   ```

2. **检查服务是否运行**：
   ```bash
   curl -I http://10.119.43.216:8767
   ```

3. **检查网络连接**：
   ```bash
   ping 10.119.43.216
   ```

### 6. 恢复原始配置

如果需要恢复原始配置：

```bash
# 恢复 browser.py
cp /home/oppo/.local/share/pipx/venvs/mcp-feedback-enhanced/lib/python3.12/site-packages/mcp_feedback_enhanced/web/utils/browser.py.backup \
   /home/oppo/.local/share/pipx/venvs/mcp-feedback-enhanced/lib/python3.12/site-packages/mcp_feedback_enhanced/web/utils/browser.py

# 删除环境变量（从 ~/.zshrc 中手动删除相关行）
```

## 总结

- ✅ 修改了浏览器打开逻辑，在远程环境中打印 URL
- ✅ 配置了固定 IP (10.119.43.216) 和端口 (8767)
- ✅ 环境变量已添加到 shell 配置文件
- 📝 需要重启 MCP 服务器以应用配置
- 🌐 使用时手动在浏览器中访问打印的 URL

## 快速访问

下次使用时，直接在浏览器中访问：

**http://10.119.43.216:8767**