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

### Method 1: Using cargo xtask (Recommended)

```bash
# 1. Clone the project
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 2. Build and package
cargo xtask build --release

# 3. Start the service
cd build/dist
./bin/starrocks-admin.sh start

# 4. Access the application
open http://localhost:8080
```

> **New Build System**: We now use `cargo xtask` for a unified, cross-platform build experience. See [xtask/README.md](xtask/README.md) for details.

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

### StarRocks User Permissions (Important)

**Before adding a cluster**, you need to create a dedicated monitoring user with appropriate read-only permissions in StarRocks.

```bash
# Execute the permission setup script
cd scripts
mysql -h <fe_host> -P 9030 -u root -p < setup_starrocks_admin_role.sql

# Verify permissions
mysql -h <fe_host> -P 9030 -u starrocks_monitor -p < verify_permissions.sql
```

For detailed permission configuration guide, see [scripts/README_PERMISSIONS.md](scripts/README_PERMISSIONS.md)

**Security Note:** Do NOT use the `root` account in production. Always create a dedicated monitoring user with minimal required permissions.

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

# Metrics collector configuration
[metrics]
interval_secs = "30s"   
retention_days = "7d"  
enabled = true          
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
![功能卡片](docs/images/4.功能卡片.png)
系统级别的配置管理，包括用户权限、系统函数等功能。还支持自定义SQL。

## 配置说明

### StarRocks 用户权限配置(重要)

**在添加集群之前**,需要在 StarRocks 中创建专用的监控用户并授予适当的只读权限。

```bash
# 执行权限配置脚本
cd scripts
mysql -h <fe_host> -P 9030 -u root -p < setup_starrocks_admin_role.sql

# 验证权限配置
mysql -h <fe_host> -P 9030 -u starrocks_monitor -p < verify_permissions.sql
```

详细的权限配置指南请参考 [scripts/README_PERMISSIONS.md](scripts/README_PERMISSIONS.md)

**安全提示:** 生产环境禁止使用 `root` 账号，务必创建专用的监控账号并遵循最小权限原则。

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

# Metrics collector configuration
# 支持人类可读格式："30s"、"5m"、"1h"；保留期支持："7d"、"2w"
[metrics]
interval_secs = "30s"    # 采集间隔，默认30秒
retention_days = "7d"    # 数据保留时长，默认7天
enabled = true            # 是否启用采集
```

- 环境变量覆盖示例：
```
APP_METRICS_INTERVAL_SECS=1m \
APP_METRICS_RETENTION_DAYS=14d \
APP_METRICS_ENABLED=true \
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

## 捐赠支持

<div align="center">

![捐赠二维码](docs/images/wx.png)

**您的捐赠将帮助我持续开源更新，非常感谢。**

---

**Made with ❤️ for StarRocks Community**

[↑ 回到顶部](#starrocks-admin)

</div>