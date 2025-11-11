# Minikube 本地测试指南

本指南说明如何在 minikube 环境中测试 StarRocks Admin 的二级路径部署。

## 前置条件

1. 已安装并启动 minikube
2. 已启用 ingress 插件
3. 已构建 Docker 镜像

## 步骤

### 1. 启动 minikube 并启用 ingress

```bash
# 启动 minikube
minikube start

# 启用 ingress 插件
minikube addons enable ingress
```

### 2. 获取 minikube IP

```bash
minikube ip
# 输出示例: 192.168.49.2
```

### 3. 配置本地 hosts

编辑 `/etc/hosts`（Linux/Mac）或 `C:\Windows\System32\drivers\etc\hosts`（Windows），添加：

```
<minikube-ip> starrocks.oppoer.me
```

例如：
```
192.168.49.2 starrocks.oppoer.me
```

### 4. 修改 Ingress 配置

编辑 `deploy/k8s/deploy-all.yaml`，将 Ingress 的 host 从 `your-domain.com` 改为 `starrocks.oppoer.me`：

```yaml
spec:
  rules:
  - host: starrocks.oppoer.me  # 修改这里
    http:
      paths:
      - pathType: Prefix
        path: /starrocks-admin(/|$)(.*)
```

### 5. 构建并加载 Docker 镜像

```bash
# 构建镜像（在项目根目录）
make docker-build

# 或者手动构建
docker build -t starrocks-admin:latest -f deploy/docker/Dockerfile .

# 加载镜像到 minikube
minikube image load starrocks-admin:latest
```

### 6. 部署到 minikube

```bash
# 应用配置
kubectl apply -f deploy/k8s/deploy-all.yaml

# 等待 Pod 就绪
kubectl wait --for=condition=ready pod -l app=starrocks-admin -n starrocks-admin --timeout=300s

# 检查 Ingress
kubectl get ingress -n starrocks-admin
```

### 7. 访问应用

在浏览器中访问：
```
http://starrocks.oppoer.me/starrocks-admin/
```

### 8. 验证修复

打开浏览器开发者工具（F12），检查：

1. **Network 标签**：
   - 所有资源（JS、CSS、图片）应该从 `/starrocks-admin/` 路径加载
   - 不应该出现 404 错误

2. **Console 标签**：
   - 不应该有资源加载错误

3. **Elements 标签**：
   - 检查 `<base href="./">` 是否正确设置

### 9. 调试命令

```bash
# 查看 Pod 日志
kubectl logs -f -l app=starrocks-admin -n starrocks-admin

# 查看 Ingress 详情
kubectl describe ingress starrocks-admin -n starrocks-admin

# 进入 Pod 调试
kubectl exec -it -n starrocks-admin $(kubectl get pod -l app=starrocks-admin -n starrocks-admin -o jsonpath='{.items[0].metadata.name}') -- /bin/sh

# 测试服务是否正常
kubectl port-forward -n starrocks-admin svc/starrocks-admin 8080:8080
# 然后访问 http://localhost:8080
```

### 10. 清理

```bash
# 删除部署
kubectl delete -f deploy/k8s/deploy-all.yaml

# 或者删除命名空间（会删除所有资源）
kubectl delete namespace starrocks-admin
```

## 常见问题

### 问题1: Ingress 无法访问

**症状**：访问 `http://starrocks.oppoer.me/starrocks-admin/` 返回 404 或无法连接

**解决方案**：
```bash
# 检查 ingress controller 是否运行
kubectl get pods -n ingress-nginx

# 检查 ingress 状态
kubectl get ingress -n starrocks-admin

# 查看 ingress controller 日志
kubectl logs -n ingress-nginx -l app.kubernetes.io/component=controller
```

### 问题2: 资源加载 404

**症状**：页面可以访问，但 JS/CSS 文件返回 404

**解决方案**：
1. 确认前端构建时使用了 `--base-href ./`
2. 检查构建后的 `frontend/dist/index.html`，确认 `<base href="./">`
3. 检查浏览器 Network 标签，确认资源请求路径是否正确

### 问题3: API 请求失败

**症状**：页面加载正常，但 API 请求返回 404

**解决方案**：
1. 确认 `environment.prod.ts` 中 `apiUrl: './api'` 是相对路径
2. 检查 Ingress 的 rewrite 规则是否正确
3. 查看后端日志确认请求是否到达

## 参考

- [Minikube Ingress 文档](https://minikube.sigs.k8s.io/docs/handbook/addons/ingress-dns/)
- [Nginx Ingress Controller 文档](https://kubernetes.github.io/ingress-nginx/)

