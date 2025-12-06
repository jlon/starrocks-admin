# 版本发布流程指南

本文档描述 StarRocks Admin 的标准发布流程。

---

## 📋 发布前准备

### 1. 确定版本号

遵循 [语义化版本](https://semver.org/) 规范：

- **Major (X.0.0)**: 不兼容的API变更
- **Minor (x.Y.0)**: 向后兼容的功能新增
- **Patch (x.y.Z)**: 向后兼容的问题修复

示例：`1.2.3`

---

### 2. 更新版本号

**必须同步更新以下4个文件**（版本一致性检查会验证）：

#### backend/Cargo.toml
```toml
[package]
name = "starrocks-admin-backend"
version = "1.2.3"  # ← 更新这里
```

#### frontend/package.json
```json
{
  "name": "starrocks-admin-frontend",
  "version": "1.2.3",  // ← 更新这里
  ...
}
```

#### deploy/chart/Chart.yaml
```yaml
apiVersion: v2
name: starrocks-admin
version: 1.2.3  # ← 更新这里
appVersion: "1.2.3"  # ← 也更新这里
```

#### CHANGELOG.md
```markdown
## [1.2.3] - 2024-12-06

### Added
- 新功能描述

### Changed
- 改进说明

### Fixed
- 修复的问题
```

---

### 3. 更新 CHANGELOG.md

遵循 [Keep a Changelog](https://keepachangelog.com/) 格式：

```markdown
# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [1.2.3] - 2024-12-06

### Added
- 新增功能A
- 新增功能B

### Changed
- 改进了X功能
- 优化了Y性能

### Fixed
- 修复了Z问题
- 解决了W bug

### Security
- 修复了安全漏洞（如有）

## [1.2.2] - 2024-11-20
...
```

**注意**:
- 日期格式：`YYYY-MM-DD`
- 版本号用方括号：`[1.2.3]`
- 分类标签：Added, Changed, Deprecated, Removed, Fixed, Security

---

### 4. 本地测试

```bash
# 1. 清理旧构建
make clean

# 2. 完整构建
make build

# 3. 测试二进制
./build/dist/bin/starrocks-admin --version

# 4. 测试启动
./build/dist/bin/starrocks-admin.sh start
curl http://localhost:8080/health
./build/dist/bin/starrocks-admin.sh stop

# 5. 测试Docker构建（可选）
make docker-build
docker run -d -p 8080:8080 starrocks-admin:latest
curl http://localhost:8080/health
docker stop $(docker ps -q --filter ancestor=starrocks-admin:latest)
```

---

## 🚀 发布流程

### 步骤1: 提交版本更新

```bash
# 1. 查看修改
git status
git diff

# 2. 提交（遵循 .gitmessage 规范）
git add backend/Cargo.toml frontend/package.json deploy/chart/Chart.yaml CHANGELOG.md
git commit -m "chore(release): prepare for v1.2.3"

# 3. 推送到远程
git push origin main
```

---

### 步骤2: 等待CI检查通过

访问 GitHub Actions 页面，确认：
- ✅ CI workflow 通过（lint, test, build）
- ✅ 所有检查项都是绿色

如果失败，修复问题后重新提交。

---

### 步骤3: 创建并推送Tag

```bash
# 1. 创建tag（必须以 v 开头）
git tag v1.2.3

# 2. 推送tag到远程
git push origin v1.2.3
```

**重要**: Tag格式必须是 `v*.*.*`（如 v1.2.3），否则不会触发发布流程。

---

### 步骤4: 自动发布流程

推送tag后，GitHub Actions 会自动执行：

#### 1. 版本一致性检查
- 验证 Cargo.toml, package.json, Chart.yaml 版本号一致
- 如果不一致，流程会失败

#### 2. 创建 GitHub Release
- 自动从 CHANGELOG.md 提取对应版本的更新内容
- 创建 Release 页面

#### 3. 构建多平台二进制包（并行）
- Linux x86_64
- macOS x86_64
- macOS ARM64 (Apple Silicon)

#### 4. 构建 Docker 镜像（并行）
- 多平台：linux/amd64, linux/arm64
- 推送到 ghcr.io
- 标签：v1.2.3, v1.2, v1, latest

#### 5. 打包 Helm Chart
- 版本化的 Chart 包
- 上传到 Release Assets

---

### 步骤5: 验证发布

#### 检查 GitHub Release
访问：`https://github.com/YOUR_USERNAME/starrocks-admin/releases`

确认：
- ✅ Release 已创建
- ✅ Release 描述包含正确的 CHANGELOG 内容
- ✅ 二进制包已上传（3个tar.gz文件）
- ✅ Helm Chart 已上传（.tgz文件）

#### 检查 Docker 镜像
```bash
# 1. 拉取镜像
docker pull ghcr.io/YOUR_USERNAME/starrocks-admin:v1.2.3
docker pull ghcr.io/YOUR_USERNAME/starrocks-admin:latest

# 2. 验证版本
docker run --rm ghcr.io/YOUR_USERNAME/starrocks-admin:v1.2.3 --version

# 3. 测试运行
docker run -d -p 8080:8080 ghcr.io/YOUR_USERNAME/starrocks-admin:v1.2.3
curl http://localhost:8080/health
```

#### 检查 Helm Chart
```bash
# 下载并验证
wget https://github.com/YOUR_USERNAME/starrocks-admin/releases/download/v1.2.3/starrocks-admin-1.2.3.tgz
helm template test starrocks-admin-1.2.3.tgz
```

---

## 🔧 故障排查

### 问题1: 版本一致性检查失败

**错误信息**:
```
❌ Version mismatch: Tag (1.2.3) != Cargo.toml (1.2.2)
```

**解决方案**:
1. 删除远程tag：`git push origin :refs/tags/v1.2.3`
2. 删除本地tag：`git tag -d v1.2.3`
3. 修复版本号不一致的文件
4. 提交修复：`git commit -am "fix(release): correct version numbers"`
5. 重新打tag并推送

---

### 问题2: CHANGELOG 提取失败

**症状**: Release 描述中没有显示更新内容

**原因**: CHANGELOG.md 格式不正确

**解决方案**:
1. 确保版本号格式：`## [1.2.3] - 2024-12-06`
2. 确保有下一个版本标题（或文件结尾）
3. 手动编辑 Release 描述

---

### 问题3: Docker 构建失败

**常见原因**:
- 前端构建失败
- 后端编译错误
- 依赖下载超时

**解决方案**:
1. 查看 Actions 日志
2. 本地复现：`make docker-build`
3. 修复问题后重新打tag

---

### 问题4: 需要重新发布

如果发布后发现问题需要重新发布：

```bash
# 1. 删除远程tag
git push origin :refs/tags/v1.2.3

# 2. 删除本地tag
git tag -d v1.2.3

# 3. 删除 GitHub Release（手动在网页上删除）

# 4. 修复问题并提交

# 5. 重新打tag
git tag v1.2.3
git push origin v1.2.3
```

---

## 📝 发布检查清单

### 发布前
- [ ] 确定版本号（遵循语义化版本）
- [ ] 更新 backend/Cargo.toml
- [ ] 更新 frontend/package.json
- [ ] 更新 deploy/chart/Chart.yaml
- [ ] 更新 CHANGELOG.md
- [ ] 本地构建测试通过
- [ ] 提交版本更新
- [ ] CI 检查通过

### 发布中
- [ ] 创建并推送 tag
- [ ] 版本一致性检查通过
- [ ] Release workflow 执行成功
- [ ] Docker workflow 执行成功

### 发布后
- [ ] GitHub Release 创建成功
- [ ] Release 描述正确
- [ ] 二进制包已上传（3个）
- [ ] Docker 镜像可拉取
- [ ] Helm Chart 已上传
- [ ] 更新 README.md（如需要）
- [ ] 通知用户（如需要）

---

## 🎯 快速参考

### 完整发布命令

```bash
# 1. 更新版本号（手动编辑4个文件）
vim backend/Cargo.toml frontend/package.json deploy/chart/Chart.yaml CHANGELOG.md

# 2. 本地测试
make clean && make build
./build/dist/bin/starrocks-admin.sh start
curl http://localhost:8080/health
./build/dist/bin/starrocks-admin.sh stop

# 3. 提交
git add backend/Cargo.toml frontend/package.json deploy/chart/Chart.yaml CHANGELOG.md
git commit -m "chore(release): prepare for v1.2.3"
git push origin main

# 4. 等待CI通过，然后打tag
git tag v1.2.3
git push origin v1.2.3

# 5. 等待自动发布完成（约10-15分钟）

# 6. 验证
docker pull ghcr.io/YOUR_USERNAME/starrocks-admin:v1.2.3
```

---

## 📚 相关文档

- [CI/CD 改进说明](CI_CD_IMPROVEMENTS.md)
- [CHANGELOG.md](../CHANGELOG.md)
- [Keep a Changelog](https://keepachangelog.com/)
- [Semantic Versioning](https://semver.org/)

---

**维护者**: StarRocks Admin Team  
**最后更新**: 2024-12-06
