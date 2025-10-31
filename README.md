# StarRocks Admin

<div align="center">

![Build Status](https://img.shields.io/badge/build-passing-brightgreen)
![License](https://img.shields.io/badge/license-Apache%202.0-blue)
![Version](https://img.shields.io/badge/version-0.1.0-orange)
![Rust](https://img.shields.io/badge/rust-1.75+-red)
![Angular](https://img.shields.io/badge/angular-15+-red)

**A modern, beautiful, and intelligent StarRocks cluster management platform**

[Features](#features) • [Quick Start](#quick-start) • [Deployment](#deployment) • [API Documentation](#api-documentation) • [Contributing](#contributing)

[中文版](#中文版) | [English](#english)

</div>

## Introduction

StarRocks Admin is a professional, enterprise-grade StarRocks database cluster management tool that provides an intuitive web interface for managing and monitoring multiple StarRocks clusters. Compared to StarRocks' native management interface, this platform offers richer functionality and a better user experience.

### Core Features

- **One-Click Deployment** - Supports traditional deployment, Docker, and Kubernetes
- **Real-time Monitoring** - View real-time cluster status and performance metrics
- **Cluster Management** - Unified management of multiple StarRocks clusters
- **Modern UI** - Modern interface based on Angular + Nebular
- **Security Authentication** - JWT authentication and permission management
- **Performance Analysis** - Query performance analysis and optimization suggestions

## Quick Start

### Method 1: One-Click Deployment (Recommended)

```bash
# 1. Clone the project
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 2. Build and package
make build

# 3. Start the service
cd build/dist
./bin/starrocks-admin.sh start

# 4. Access the application
open http://localhost:8080
```

### Method 2: Docker Deployment

```bash
# 1. Clone the project
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 2. Start the service
make docker-build  # Build Docker image
make docker-up     # Start Docker container

# 3. Access the application
open http://localhost:8080
```

## Interface Preview

StarRocks Admin provides an intuitive and beautiful web management interface covering all aspects of cluster management.

### Cluster Overview
![Cluster Overview](docs/images/1集群概览.png)
Real-time display of overall cluster status, performance metrics, and resource usage for a comprehensive view of cluster health.

### Cluster Management
![Cluster Management](docs/images/1集群列表.png)
Unified management of multiple StarRocks clusters with support for adding, editing, and deleting cluster configurations.

### Node Management
![Node Management](docs/images/2节点管理.png)
View and manage all nodes in the cluster, monitoring the running status and resource usage of each node.

### Query Management
![Query Management](docs/images/3查询管理.png)
Real-time view of executing queries with support for query termination and performance analysis.

### Materialized Views
![Materialized Views](docs/images/9.物化视图.png)
View and manage all materialized views in the cluster, with support for enabling, disabling, and editing.

### Session Management
![Session Management](docs/images/6回话管理.png)
Manage database connection sessions, view active sessions and historical connection information.

### Variable Management
![Variable Management](docs/images/5变量管理.png)
Configure and manage system variables with support for viewing and modifying runtime parameters.

### Feature Cards
![System Management](docs/images/4.功能卡片.png)
System-level configuration management including user permissions, system functions, and more. Also supports custom SQL.

## Configuration

### Main Configuration File (conf/config.toml)

```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
url = "sqlite://data/starrocks-admin.db"

[auth]
jwt_secret = "your-secret-key-change-in-production"
jwt_expires_in = "24h"

[logging]
level = "info,starrocks_admin_backend=debug"
file = "logs/starrocks-admin.log"

[static_config]
enabled = true
web_root = "web"
```

## Development

### Hot Reload Development

For rapid development iteration, the project supports hot reload:

```bash
# Development environment with separate terminal windows
make dev

# Or start in background mode
make dev-start

# View service status
make dev-status

# View real-time logs
make dev-logs

# Stop services
make dev-stop

# Restart services
make dev-restart
```

### Development Features

- **Optimized Build**: Incremental compilation with dependency caching
- **Hot Reload**: Code changes automatically reload without manual restart
- **Backend**: Uses `cargo-watch` for automatic Rust code recompilation
- **Frontend**: Angular development server with built-in hot reload
- **Fast Iteration**: Modify code → Save → Instant preview
- **Smart Caching**: Third-party libraries are pre-compiled and cached

### Quick Commands

```bash
# One command to start everything with hot reload
make dev

# The development environment will be available at:
# - Frontend: http://localhost:4200
# - Backend:  http://localhost:8081
# - API Docs: http://localhost:8081/api-docs
```

## Contributing

We welcome all forms of contributions! Please follow these steps:

1. **Fork the project**
2. **Create a feature branch** (`git checkout -b feature/AmazingFeature`)
3. **Commit your changes** (`git commit -m 'Add some AmazingFeature'`)
4. **Push to the branch** (`git push origin feature/AmazingFeature`)
5. **Create a Pull Request**

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [ngx-admin](https://github.com/John/ngx-admin) - Excellent Angular admin template
- [Nebular](https://John.github.io/nebular/) - Beautiful UI component library
- [Axum](https://github.com/tokio-rs/axum) - Powerful Rust web framework
- [StarRocks](https://www.starrocks.io/) - High-performance analytical database
---
[↑ Back to Top](#starrocks-admin)
---

# 中文版

<div align="center">

**一个现代化、美观、智能的 StarRocks 集群管理平台**

[功能特性](#功能特性) • [快速开始](#快速开始) • [部署指南](#部署指南) • [API 文档](#api-文档) • [贡献](#贡献)

[English](#english) | [中文版](#中文版)

</div>

## 简介

StarRocks Admin 是一个专业的、企业级的 StarRocks 数据库集群管理工具，提供直观的 Web 界面来管理和监控多个 StarRocks 集群。相比 StarRocks 原生的管理界面，本平台提供了更丰富的功能和更好的用户体验。

### 核心特性

- **一键部署** - 支持传统部署、Docker 和 Kubernetes
- **实时监控** - 查看集群的实时状态和性能指标
- **集群管理** - 统一管理多个 StarRocks 集群
- **现代 UI** - 基于 Angular + Nebular 的现代化界面
- **安全认证** - JWT 认证和权限管理
- **性能分析** - 查询性能分析和优化建议

## 快速开始

### 方式一：一键部署（推荐）

```bash
# 1. 克隆项目
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 2. 构建和打包
make build

# 3. 启动服务
cd build/dist
./bin/starrocks-admin.sh start

# 4. 访问应用
open http://localhost:8080
```

### 方式二：Docker 部署

```bash
# 1. 克隆项目
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 2. 启动服务
make docker-build  # 构建 Docker 镜像
make docker-up     # 启动 Docker 容器

# 3. 访问应用
open http://localhost:8080
```

## 界面预览

StarRocks Admin 提供了直观、美观的 Web 管理界面，涵盖集群管理的各个方面。

### 集群概览
![集群概览](docs/images/1集群概览.png)
实时展示集群整体状态、性能指标和资源使用情况，一目了然掌握集群健康状态。

### 集群管理
![集群管理](docs/images/1集群列表.png)
统一管理多个 StarRocks 集群，支持添加、编辑、删除集群配置。

### 节点管理
![节点管理](docs/images/2节点管理.png)
查看和管理集群中的所有节点，监控各节点的运行状态和资源使用。

### 查询管理
![查询管理](docs/images/3查询管理.png)
实时查看正在执行的查询，支持查询终止和性能分析。

### 物化视图
![节点管理](docs/images/9.物化视图.png)
查看和管理集群中的所有物化视图，可以开启关闭编辑等。

### 会话管理
![会话管理](docs/images/6回话管理.png)
管理数据库连接会话，查看活跃会话和历史连接信息。

### 变量管理
![变量管理](docs/images/5变量管理.png)
配置和管理系统变量，支持查看和修改运行时参数。

### 功能卡片
![系统管理](docs/images/4.功能卡片.png)
系统级别的配置管理，包括用户权限、系统函数等功能。还支持自定义SQL。

## 配置说明

### 主配置文件 (conf/config.toml)

```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
url = "sqlite://data/starrocks-admin.db"

[auth]
jwt_secret = "your-secret-key-change-in-production"
jwt_expires_in = "24h"

[logging]
level = "info,starrocks_admin_backend=debug"
file = "logs/starrocks-admin.log"

[static_config]
enabled = true
web_root = "web"
```

## 日志配置说明（后端）

> 为了安全和环境灵活性，后端 `.env.development`、`.env.production` 日志配置文件未纳入仓库，请**按需在 backend 目录下自行创建**。

- **开发环境**：彩色格式化日志，利于本地调试
- **生产环境**：结构化 JSON 日志，方便聚合收集

**范例：backend/.env.development**
```env
LOG_FORMAT=pretty
RUST_LOG=starrocks_admin=debug,tower_http=debug
RUST_LOG_STYLE=always
```
**范例：backend/.env.production**
```env
LOG_FORMAT=json
RUST_LOG=starrocks_admin=info,tower_http=warn
RUST_LOG_STYLE=never
```

**切换方法：**
- 开发调试：`cp .env.development .env`
- 生产上线：`cp .env.production .env`

本项目已自动识别并加载 `.env` 文件，无需手动 export 环境变量。

## 开发指南

### 热重载开发

为了支持快速开发迭代，项目支持热重载：

```bash
# 启动开发环境（前后端分离终端窗口）
make dev

# 或以后台模式启动
make dev-start

# 查看服务状态
make dev-status

# 查看实时日志
make dev-logs

# 停止服务
make dev-stop

# 重启服务
make dev-restart
```

### 开发特性

- **优化构建**：增量编译与依赖缓存，大幅提升编译速度
- **热重载**：代码修改自动重新加载，无需手动重启
- **后端**：使用 `cargo-watch` 实现 Rust 代码自动重新编译
- **前端**：Angular 开发服务器自带热重载
- **快速迭代**：修改代码 → 保存 → 即时预览
- **智能缓存**：第三方库预编译缓存，避免重复编译

### 快速命令

```bash
# 一个命令启动所有服务（前后端分离终端窗口，方便查看日志）
make dev

# 开发环境将运行在：
# - 前端: http://localhost:4200
# - 后端:  http://localhost:8081
# - API文档: http://localhost:8081/api-docs
```

## 贡献

我们欢迎所有形式的贡献！请遵循以下步骤：

1. **Fork 项目**
2. **创建特性分支** (`git checkout -b feature/AmazingFeature`)
3. **提交更改** (`git commit -m 'Add some AmazingFeature'`)
4. **推送分支** (`git push origin feature/AmazingFeature`)
5. **创建 Pull Request**

## 许可证

本项目采用 Apache License 2.0 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 致谢

- [ngx-admin](https://github.com/John/ngx-admin) - 优秀的 Angular 管理模板
- [Nebular](https://John.github.io/nebular/) - 漂亮的 UI 组件库
- [Axum](https://github.com/tokio-rs/axum) - 强大的 Rust Web 框架
- [StarRocks](https://www.starrocks.io/) - 高性能分析数据库

## 修改记录

### 2025-10-31: 功能分支合并 - 日志优化、查询修复、构建优化

#### 会话主要目的
- 将三个功能分支合并到主分支
- 日志格式优化功能
- 运行中查询功能修复
- Rust 编译速度优化

#### 完成的主要任务
1. **日志格式优化** (`feature/log-format-improvement`)
   - 支持开发/生产环境日志格式切换（LOG_FORMAT 环境变量）
   - 集成 dotenvy 自动加载 .env 文件
   - 实现美化彩色日志（开发）与结构化 JSON 日志（生产）
   - 添加日志配置指南文档

2. **查询功能修复** (`feature/fix-running-queries`)
   - 修复运行中的查询功能列名匹配问题（ScanRows vs ProcessRows）
   - 使用大小写不敏感匹配查找列名
   - 将 Sql 列设为可选，使用 ExecProgress 作为备用
   - 添加 HTTP API 和 MySQL 客户端双重备用方案
   - 修复 Angular CommonJS 警告（添加 nearley 和 sql-formatter）

3. **构建优化** (`feature/build-optimization`)
   - 自动检测 CPU 核心数并设置并行任务数（使用 75% 核心，最多 14 个）
   - 优化 codegen-units 从 1 改为 16，平衡编译速度和代码质量
   - 改进启动脚本，自动设置 CARGO_BUILD_JOBS 环境变量
   - 添加详细的编译速度优化指南文档

4. **分支管理实践**
   - 创建 Git 分支管理实战指南文档
   - 记录提交拆分与分支重组的完整流程

#### 关键决策和解决方案
- **分支分离**：确保每个功能分支主题单一，避免功能混杂
- **合并策略**：使用 Fast-forward 和 Merge commit 相结合的策略
- **文档完善**：为分支管理操作创建实战指南，便于团队参考

#### 使用的技术栈
- Rust: cargo build, cargo watch, 编译优化配置
- Angular: CommonJS 依赖配置
- Git: 分支管理、提交拆分、合并策略
- StarRocks: SHOW PROC 命令解析

#### 修改的文件

**日志优化相关：**
- `backend/src/main.rs` - 集成 dotenvy 和日志初始化
- `backend/Cargo.toml` - 更新 tracing 依赖
- `backend/.env.development` - 开发环境日志配置（新建）
- `backend/.env.production` - 生产环境日志配置（新建）
- `README.md` - 添加日志配置指南

**查询功能修复：**
- `backend/src/handlers/query.rs` - 修复列名匹配和解析逻辑
- `backend/src/services/starrocks_client.rs` - 改进查询解析和错误处理
- `frontend/angular.json` - 修复 CommonJS 警告

**构建优化：**
- `backend/.cargo/config.toml` - 优化编译配置
- `scripts/dev/start_backend_dev_optimized.sh` - 编译优化脚本
- `dev-doc/编译速度优化.md` - 优化指南文档（新建）

**文档：**
- `dev-doc/Git分支管理实战指南-提交拆分与分支重组.md` - 分支管理实战指南（新建）

### 2025-01-25: 优化构建系统和修复代码质量问题

#### 会话主要目的
- 修复 cargo 文件锁检测逻辑问题
- 优化构建系统代码结构
- 修复业务代码质量问题
- 分批提交构建代码和业务代码

#### 完成的主要任务
1. **修复 cargo 进程检测逻辑**
   - 移除了不准确的 `pgrep` 进程检测
   - 改为直接编译 + 超时保护机制
   - 让 cargo 自动处理文件锁等待

2. **代码质量检查与修复**
   - 修复 Rust Clippy 警告（collapsible if, needless borrow, get(0)）
   - 修复 Angular Lint 警告（OnInit 接口实现）
   - 优化 Rust 代码格式

3. **分批代码提交**
   - 第一批：构建系统代码（Makefile、脚本、配置文件等）
   - 第二批：业务代码（backend/src、frontend/src）

#### 关键决策和解决方案
- **文件锁处理**：信任 cargo 内置的文件锁机制，不再手动检测进程，避免误判
- **超时保护**：添加 5 分钟编译超时保护，防止无限等待
- **代码分离提交**：将构建代码和业务代码分开提交，便于代码审查和维护

#### 使用的技术栈
- Rust: cargo build, cargo watch, cargo clippy, cargo fmt
- Angular: ng lint, prettier
- Git: 分批提交策略

#### 修改的文件

**构建系统文件（第一批提交）：**
- `Makefile` - 优化构建命令复用
- `.gitignore` - 更新忽略规则
- `backend/.cargo/config.toml` - Cargo 配置优化
- `scripts/dev/start_backend.sh` - 修复文件锁检测逻辑
- `scripts/dev/check_status.sh` - 服务状态检测脚本
- `scripts/dev/common.sh` - WSL 环境检测公共函数
- `scripts/dev/start_separate_terminals.sh` - 分离终端启动脚本
- `scripts/dev/stop.sh` - 服务停止脚本优化
- `scripts/dev/incremental_build.sh` - 增量构建脚本
- `scripts/dev/install_nodejs.sh` - Node.js 安装脚本
- `scripts/config/generate-frontend-environments.js` - 前端环境配置生成
- `scripts/quality-check.sh` - 代码质量检查脚本
- `dev-doc/开发环境优化说明.md` - 开发环境优化文档
- `dev-doc/热重载优化说明.md` - 热重载优化文档
- `README.md` - 更新文档说明

**业务代码文件（第二批提交）：**
- `backend/src/config.rs` - 修复 Clippy 警告
- `backend/src/handlers/overview.rs` - 代码优化
- `backend/src/services/mod.rs` - 代码优化
- `backend/src/services/overview_service.rs` - 修复 Clippy 警告和代码格式
- `frontend/src/app/pages/starrocks/cluster-overview/metric-card-group/metric-card-group.component.ts` - 修复 Angular Lint 警告
- `frontend/src/environments/environment.ts` - 环境配置更新
- `frontend/src/environments/environment.prod.ts` - 生产环境配置更新

## 捐赠支持

<div align="center">

![捐赠二维码](docs/images/wx.png)

**您的捐赠将帮助我持续开源更新，非常感谢。**

---

**Made with ❤️ for StarRocks Community**

[↑ 回到顶部](#starrocks-admin)

</div>