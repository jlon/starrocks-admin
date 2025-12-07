# StarRocks Admin 部署指南

本文档详细介绍 StarRocks Admin 的多种部署方式，包括本地部署、Docker 部署、Kubernetes YAML 部署和 Helm Chart 部署。

## 目录

- [系统要求](#系统要求)
- [部署方式概览](#部署方式概览)
- [一、本地部署](#一本地部署)
- [二、Docker 部署](#二docker-部署)
- [三、Kubernetes YAML 部署](#三kubernetes-yaml-部署)
- [四、Helm Chart 部署](#四helm-chart-部署)
- [配置说明](#配置说明)
- [常见问题](#常见问题)

---

## 系统要求

| 项目 | 最低要求 | 推荐配置 |
|------|---------|---------|
| CPU | 1 核 | 2 核+ |
| 内存 | 256MB | 1GB+ |
| 磁盘 | 50Mi | 1GB+ |
| 操作系统 | Linux x86_64 | Linux x86_64 / macOS (Docker) |

### 软件依赖

- **本地部署**: Rust 1.75+, Node.js 18+
- **Docker 部署**: Docker 20.10+, Docker Compose 2.0+
- **Kubernetes 部署**: Kubernetes 1.16+, kubectl
- **Helm 部署**: Helm 3.0+, Kubernetes 1.16+

---

## 部署方式概览

| 部署方式 | 适用场景 | 复杂度 | 推荐指数 |
|---------|---------|--------|---------|
| 本地部署 | 开发测试、单机环境 | ⭐⭐ | ⭐⭐⭐ |
| Docker 部署 | 快速体验、单机生产 | ⭐ | ⭐⭐⭐⭐⭐ |
| K8s YAML 部署 | 简单 K8s 环境 | ⭐⭐⭐ | ⭐⭐⭐ |
| Helm 部署 | 生产级 K8s 环境 | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

---

## 一、本地部署

本地部署适合开发测试环境，需要从源码编译构建。

### 1.1 环境准备

```bash
# 安装 Rust (如果未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 安装 Node.js 18+ (推荐使用 nvm)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
nvm use 18

# 验证安装
rustc --version   # 应显示 1.75+
node --version    # 应显示 v18+
npm --version
```

### 1.2 克隆项目

```bash
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin
```

### 1.3 构建项目

```bash
# 一键构建（推荐）
make build

# 构建完成后，产物位于 build/dist 目录
```

构建过程会自动完成以下步骤：
1. 构建前端 (Angular + Nebular)
2. 运行 Rust clippy 代码检查
3. 构建后端 (Rust + Axum)
4. 打包分发文件

### 1.4 启动服务

```bash
cd build/dist

# 启动服务
./bin/starrocks-admin.sh start

# 查看状态
./bin/starrocks-admin.sh status

# 停止服务
./bin/starrocks-admin.sh stop

# 重启服务
./bin/starrocks-admin.sh restart
```

### 1.5 访问应用

打开浏览器访问：http://localhost:8080

### 1.6 目录结构

```
build/dist/
├── bin/                    # 可执行文件
│   ├── starrocks-admin     # 主程序
│   └── starrocks-admin.sh  # 启动脚本
├── conf/                   # 配置文件
│   └── config.toml         # 主配置文件
├── data/                   # 数据目录 (SQLite 数据库)
├── logs/                   # 日志目录
├── lib/                    # 依赖库
└── migrations/             # 数据库迁移文件
```

---

## 二、Docker 部署

Docker 部署是最简单快捷的方式，推荐用于快速体验和单机生产环境。

### 2.1 使用预构建镜像（推荐）

```bash
# 拉取最新镜像
docker pull ghcr.io/jlon/starrocks-admin:latest

# 创建数据目录
mkdir -p ./data ./logs

# 启动容器
docker run -d \
  --name starrocks-admin \
  -p 8080:8080 \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/logs:/app/logs \
  --restart unless-stopped \
  ghcr.io/jlon/starrocks-admin:latest
```

### 2.2 使用指定版本

```bash
# 拉取指定版本
docker pull ghcr.io/jlon/starrocks-admin:1.0.0

# 启动容器
docker run -d \
  --name starrocks-admin \
  -p 8080:8080 \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/logs:/app/logs \
  ghcr.io/jlon/starrocks-admin:1.0.0
```

### 2.3 从源码构建镜像

```bash
# 克隆项目
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 构建镜像
make docker-build

# 或手动构建
docker build -f deploy/docker/Dockerfile -t starrocks-admin:latest .
```

### 2.4 使用 Docker Compose

```bash
cd starrocks-admin

# 启动服务
make docker-up

# 或直接使用 docker compose
cd deploy/docker
docker compose up -d

# 停止服务
make docker-down
```

**docker-compose.yml 配置示例：**

```yaml
services:
  starrocks-admin:
    image: ghcr.io/jlon/starrocks-admin:latest
    container_name: starrocks-admin
    ports:
      - "8080:8080"
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
```

### 2.5 自定义配置

通过环境变量覆盖默认配置：

```bash
docker run -d \
  --name starrocks-admin \
  -p 8080:8080 \
  -e APP_SERVER_PORT=8080 \
  -e APP_JWT_SECRET="your-secure-jwt-secret" \
  -e APP_LOG_LEVEL="info,starrocks_admin=debug" \
  -e APP_METRICS_ENABLED=true \
  -e APP_METRICS_INTERVAL_SECS=30s \
  -e APP_METRICS_RETENTION_DAYS=7d \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/logs:/app/logs \
  ghcr.io/jlon/starrocks-admin:latest
```

### 2.6 常用 Docker 命令

```bash
# 查看日志
docker logs -f starrocks-admin

# 进入容器
docker exec -it starrocks-admin /bin/sh

# 查看容器状态
docker ps -a | grep starrocks-admin

# 停止并删除容器
docker stop starrocks-admin && docker rm starrocks-admin
```

---

## 三、Kubernetes YAML 部署

适合简单的 Kubernetes 环境，使用原生 YAML 文件部署。

### 3.1 前置条件

- Kubernetes 集群 1.16+
- kubectl 已配置并连接到集群
- Ingress Controller（如需外部访问）

### 3.2 快速部署

```bash
# 克隆项目
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 一键部署所有资源
kubectl apply -f deploy/k8s/deploy-all.yaml

# 等待 Pod 就绪
kubectl wait --for=condition=ready pod -l app=starrocks-admin -n starrocks-admin --timeout=300s
```

### 3.3 部署文件说明

`deploy/k8s/deploy-all.yaml` 包含以下资源：

| 资源类型 | 名称 | 说明 |
|---------|------|------|
| Namespace | starrocks-admin | 独立命名空间 |
| ConfigMap | starrocks-admin-config | 应用配置 |
| Secret | starrocks-admin-secret | JWT 密钥等敏感信息 |
| StatefulSet | starrocks-admin | 应用部署（单副本） |
| Service | starrocks-admin | ClusterIP 服务 |
| Ingress | starrocks-admin | 外部访问入口 |

**存储说明**：

StatefulSet 使用 `volumeClaimTemplates` 自动创建 PVC，用于持久化 SQLite 数据库：

```yaml
volumeClaimTemplates:
- metadata:
    name: data
  spec:
    accessModes: 
      - ReadWriteOnce
    resources:
      requests:
        storage: 10Gi
```

挂载路径：
- `/app/data` - 数据库文件（PVC 持久化）
- `/app/logs` - 日志文件（emptyDir，Pod 重启后清空）
- `/app/conf` - 配置文件（ConfigMap 只读挂载）

### 3.4 修改配置

**修改 JWT 密钥（生产环境必须）：**

```yaml
# 在 deploy/k8s/deploy-all.yaml 中找到 Secret 部分
apiVersion: v1
kind: Secret
metadata:
  name: starrocks-admin-secret
  namespace: starrocks-admin
type: Opaque
stringData:
  # 生成安全密钥: openssl rand -base64 32
  jwt-secret: "YOUR-SECURE-JWT-SECRET-HERE"
```

**修改 Ingress 域名：**

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: starrocks-admin
  namespace: starrocks-admin
spec:
  rules:
  - host: your-domain.com  # 修改为你的域名
    http:
      paths:
      - pathType: Prefix
        path: /starrocks-admin(/|$)(.*)
        backend:
          service:
            name: starrocks-admin
            port:
              number: 8080
```

### 3.5 访问应用

**方式一：Port Forward（开发测试）**

```bash
kubectl port-forward -n starrocks-admin svc/starrocks-admin 8080:8080
# 访问 http://localhost:8080
```

**方式二：Ingress（生产环境）**

配置好 Ingress 后，通过配置的域名访问：
```
http://your-domain.com/starrocks-admin/
```

**方式三：NodePort**

修改 Service 类型为 NodePort：

```yaml
apiVersion: v1
kind: Service
metadata:
  name: starrocks-admin
  namespace: starrocks-admin
spec:
  type: NodePort
  selector:
    app: starrocks-admin
  ports:
  - port: 8080
    targetPort: 8080
    nodePort: 30080  # 指定 NodePort
```

### 3.6 Minikube 本地测试

```bash
# 启动 minikube
minikube start

# 启用 ingress 插件
minikube addons enable ingress

# 获取 minikube IP
minikube ip

# 配置 hosts 文件
echo "$(minikube ip) starrocks.local" | sudo tee -a /etc/hosts

# 构建并加载镜像
make docker-build
minikube image load starrocks-admin:latest

# 部署
kubectl apply -f deploy/k8s/deploy-all.yaml

# 访问
open http://starrocks.local/starrocks-admin/
```

### 3.7 常用运维命令

```bash
# 查看所有资源
kubectl get all -n starrocks-admin

# 查看 Pod 日志
kubectl logs -f -l app=starrocks-admin -n starrocks-admin

# 查看 Pod 详情
kubectl describe pod -l app=starrocks-admin -n starrocks-admin

# 进入 Pod 调试
kubectl exec -it -n starrocks-admin $(kubectl get pod -l app=starrocks-admin -n starrocks-admin -o jsonpath='{.items[0].metadata.name}') -- /bin/sh

# 删除所有资源
kubectl delete -f deploy/k8s/deploy-all.yaml

# 或删除整个命名空间
kubectl delete namespace starrocks-admin
```

---

## 四、Helm Chart 部署

Helm 部署是生产环境推荐的方式，提供更灵活的配置管理和版本控制。

### 4.1 前置条件

- Kubernetes 集群 1.16+
- Helm 3.0+
- Ingress Controller（可选）

### 4.2 安装 Helm

```bash
# macOS
brew install helm

# Linux
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# 验证安装
helm version
```

### 4.3 快速安装

**方式一：从 GitHub Release 安装 Helm Chart**

> **说明**：GitHub Release 页面包含两个独立的包：
> - `starrocks-admin-x.x.x-linux-amd64.tar.gz` - 应用程序二进制包（用于本地部署）
> - `starrocks-admin-x.x.x.tgz` - Helm Chart 包（用于 K8s 部署）
>
> Helm Chart 包只包含 Kubernetes 部署模板，应用镜像从 `ghcr.io/jlon/starrocks-admin` 拉取。

```bash
# 下载 Helm Chart 包
wget https://github.com/jlon/starrocks-admin/releases/download/v1.0.0/starrocks-admin-1.0.0.tgz

# 安装（会自动从 ghcr.io 拉取应用镜像）
helm install starrocks-admin starrocks-admin-1.0.0.tgz

# 或指定命名空间
helm install starrocks-admin starrocks-admin-1.0.0.tgz -n starrocks-admin --create-namespace
```

**方式二：从源码安装**

```bash
# 克隆项目
git clone https://github.com/jlon/starrocks-admin.git
cd starrocks-admin

# 安装到默认命名空间
helm install starrocks-admin deploy/chart

# 安装到指定命名空间
helm install starrocks-admin deploy/chart -n starrocks-admin --create-namespace
```

### 4.4 自定义配置安装

**使用 --set 参数：**

```bash
helm install starrocks-admin deploy/chart \
  --set image.tag=1.0.0 \
  --set service.type=LoadBalancer \
  --set persistence.size=20Gi \
  --set jwtSecret="your-secure-jwt-secret" \
  --set ingress.hosts[0].host=starrocks.yourdomain.com
```

**使用自定义 values 文件：**

```bash
# 创建自定义配置文件
cat > my-values.yaml << EOF
replicaCount: 1

image:
  repository: ghcr.io/jlon/starrocks-admin
  tag: "1.0.0"
  pullPolicy: IfNotPresent

service:
  type: ClusterIP
  port: 8080

ingress:
  enabled: true
  className: nginx
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /\$2
  hosts:
    - host: starrocks.yourdomain.com
      paths:
        - path: /starrocks-admin(/|$)(.*)
          pathType: Prefix
  tls:
    - secretName: starrocks-admin-tls
      hosts:
        - starrocks.yourdomain.com

persistence:
  enabled: true
  size: 20Gi
  storageClassName: "standard"

resources:
  limits:
    cpu: 1000m
    memory: 1Gi
  requests:
    cpu: 250m
    memory: 256Mi

jwtSecret: "your-secure-jwt-secret-change-in-production"
EOF

# 使用自定义配置安装
helm install starrocks-admin deploy/chart -f my-values.yaml
```

### 4.5 配置参数说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `replicaCount` | 副本数量 | `1` |
| `image.repository` | 镜像仓库 | `ghcr.io/jlon/starrocks-admin` |
| `image.tag` | 镜像标签 | `latest` |
| `image.pullPolicy` | 镜像拉取策略 | `IfNotPresent` |
| `service.type` | Service 类型 | `ClusterIP` |
| `service.port` | Service 端口 | `8080` |
| `ingress.enabled` | 启用 Ingress | `true` |
| `ingress.className` | Ingress Class | `""` |
| `ingress.hosts` | Ingress 主机配置 | 见 values.yaml |
| `persistence.enabled` | 启用持久化 | `true` |
| `persistence.size` | 存储大小 | `10Gi` |
| `persistence.storageClassName` | 存储类 | `""` |
| `resources` | 资源限制 | `{}` |
| `jwtSecret` | JWT 密钥 | 自动生成 |
| `autoscaling.enabled` | 启用自动伸缩 | `false` |
| `nodeSelector` | 节点选择器 | `{}` |
| `tolerations` | 容忍度 | `[]` |
| `affinity` | 亲和性 | `{}` |

### 4.6 升级和回滚

```bash
# 升级到新版本
helm upgrade starrocks-admin deploy/chart -f my-values.yaml

# 查看历史版本
helm history starrocks-admin

# 回滚到上一版本
helm rollback starrocks-admin

# 回滚到指定版本
helm rollback starrocks-admin 1
```

### 4.7 卸载

```bash
# 卸载 release
helm uninstall starrocks-admin

# 卸载并删除 PVC（注意：会删除数据）
helm uninstall starrocks-admin
kubectl delete pvc -l app.kubernetes.io/instance=starrocks-admin
```

### 4.8 查看部署状态

```bash
# 查看 release 状态
helm status starrocks-admin

# 查看生成的 manifest
helm get manifest starrocks-admin

# 查看 values
helm get values starrocks-admin

# 模拟安装（不实际部署）
helm install starrocks-admin deploy/chart --dry-run --debug
```

---

## 配置说明

### 主配置文件 (config.toml)

```toml
[server]
host = "0.0.0.0"          # 监听地址
port = 8080               # 监听端口

[database]
url = "sqlite://data/starrocks-admin.db"  # 数据库连接

[auth]
jwt_secret = "your-secret-key"  # JWT 密钥（生产环境必须修改）
jwt_expires_in = "24h"          # Token 过期时间

[logging]
level = "info,starrocks_admin=debug"  # 日志级别
file = "logs/starrocks-admin.log"     # 日志文件

[metrics]
interval_secs = "30s"     # 指标采集间隔
retention_days = "7d"     # 数据保留时长
enabled = true            # 是否启用采集
```

### 环境变量覆盖

所有配置项都可以通过环境变量覆盖，格式为 `APP_` 前缀 + 大写配置项：

| 配置项 | 环境变量 |
|--------|---------|
| server.host | APP_SERVER_HOST |
| server.port | APP_SERVER_PORT |
| database.url | APP_DATABASE_URL |
| auth.jwt_secret | APP_JWT_SECRET |
| logging.level | APP_LOG_LEVEL |
| metrics.enabled | APP_METRICS_ENABLED |

---

## 常见问题

### Q1: 如何生成安全的 JWT 密钥？

```bash
openssl rand -base64 32
```

### Q2: 如何查看应用日志？

```bash
# Docker
docker logs -f starrocks-admin

# Kubernetes
kubectl logs -f -l app=starrocks-admin -n starrocks-admin
```

### Q3: 如何备份数据？

```bash
# Docker
docker cp starrocks-admin:/app/data ./backup/

# Kubernetes
kubectl cp starrocks-admin/<pod-name>:/app/data ./backup/ -n starrocks-admin
```

### Q4: Ingress 访问返回 404？

1. 确认 Ingress Controller 已安装并运行
2. 检查 Ingress 配置的 host 是否正确
3. 确认 DNS 或 hosts 文件配置正确
4. 查看 Ingress Controller 日志排查问题

### Q5: 如何配置 HTTPS？

**Helm 方式：**

```yaml
ingress:
  enabled: true
  tls:
    - secretName: starrocks-admin-tls
      hosts:
        - starrocks.yourdomain.com
```

**创建 TLS Secret：**

```bash
kubectl create secret tls starrocks-admin-tls \
  --cert=path/to/tls.crt \
  --key=path/to/tls.key \
  -n starrocks-admin
```

### Q6: 如何扩容？

由于使用 SQLite 数据库，目前仅支持单副本运行。如需高可用，建议：
1. 使用外部数据库（如 PostgreSQL）
2. 配置数据库主从复制
3. 使用共享存储

---

## 附录：Release 发布机制

当推送版本标签（如 `v1.0.0`）时，GitHub Actions 会自动执行以下流程：

### 版本一致性检查

自动验证以下文件的版本号是否一致：
- `backend/Cargo.toml`
- `frontend/package.json`
- `deploy/chart/Chart.yaml`

### 发布产物

| 产物 | 文件名 | 说明 |
|------|--------|------|
| 应用程序包 | `starrocks-admin-x.x.x-linux-amd64.tar.gz` | 包含二进制文件、配置、迁移脚本等，用于本地部署 |
| Helm Chart 包 | `starrocks-admin-x.x.x.tgz` | 包含 K8s 部署模板，用于 Helm 部署 |
| Docker 镜像 | `ghcr.io/jlon/starrocks-admin:x.x.x` | 多架构镜像（amd64/arm64），由 docker-publish workflow 构建 |

### Helm Chart 发布流程

```yaml
# release.yml 中的 publish-helm-chart job
- name: Package Helm chart
  run: |
    cd deploy/chart
    helm package . --version ${{ version }}

- name: Upload Helm chart to release
  uses: softprops/action-gh-release@v1
  with:
    files: deploy/chart/starrocks-admin-${{ version }}.tgz
```

Helm Chart 包只包含部署模板（templates/、values.yaml、Chart.yaml 等），**不包含应用程序二进制文件**。部署时会从 `ghcr.io/jlon/starrocks-admin` 拉取 Docker 镜像。

---

## 相关链接

- [项目主页](https://github.com/jlon/starrocks-admin)
- [问题反馈](https://github.com/jlon/starrocks-admin/issues)
- [更新日志](../CHANGELOG.md)

---

**如有问题，欢迎通过邮件联系：itjlon@gmail.com**
