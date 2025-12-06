# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2024-12-05

### 🎉 首个正式版本发布

StarRocks Admin 是一个现代化的企业级 StarRocks 集群管理平台，提供直观的 Web 界面来管理和监控多个 StarRocks 集群。

### ✨ 核心功能

#### 集群管理
- 多集群统一管理，支持添加、编辑、删除集群配置
- 集群概览仪表板，实时展示健康状态、性能指标和资源使用

#### 节点管理
- FE（Frontend）节点管理和监控
- BE（Backend）节点管理和性能指标查看

#### 查询管理
- 实时查询监控，支持查看和终止正在执行的查询
- 查询审计日志，完整记录所有执行历史
- Query Profile 可视化分析，支持 DAG 图展示和智能优化建议

#### 数据管理
- 物化视图管理，支持查看、启用、禁用操作
- 会话管理，查看活跃连接和历史会话信息
- 变量管理，配置和修改系统运行参数

#### 系统管理
- 用户管理，支持用户的增删改查
- 角色管理，基于 RBAC 的细粒度权限控制
- 组织管理，支持多租户隔离

#### 技术特性
- 完整的中英文国际化支持
- JWT 认证和权限管理
- 指标采集服务，支持历史数据查询和性能分析
- 现代化 UI，基于 Angular + Nebular 框架
- 多种部署方式：传统部署、Docker、Kubernetes (Helm Chart)

### 📦 部署支持
- 一键部署脚本
- Docker 镜像支持（多平台：linux/amd64, linux/arm64）
- Kubernetes Helm Chart
- 多平台二进制包（Linux x86_64, macOS x86_64, macOS ARM64）

[Unreleased]: https://github.com/jlon/starrocks-admin/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jlon/starrocks-admin/releases/tag/v1.0.0
