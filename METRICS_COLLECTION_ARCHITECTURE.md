# Backend Metrics Collection Architecture

## Overview
后端通过定期采集机制从 StarRocks 集群实时收集性能和资源指标。

## Collection Flow

### 1. **启动流程** (main.rs:278)
```
Backend 启动
  ↓
初始化 MetricsCollectorService
  ↓
创建 ScheduledExecutor (周期：30秒)
  ↓
启动周期性采集任务
```

### 2. **采集周期**
- **默认间隔**: 30 秒
- **配置位置**: `backend/config/metrics-collector.toml`
- **配置字段**: `[collector].interval_secs`

### 3. **采集任务** (MetricsCollectorService::collect_once)

每个采集周期执行以下步骤：

#### Step 1: 从所有集群采集指标
```rust
pub async fn collect_all_clusters() {
  for each cluster {
    collect_cluster_metrics(cluster)
    ├─ 获取 Prometheus 指标文本
    ├─ 获取 Backends 信息
    ├─ 获取 Frontends 信息
    ├─ 获取运行时信息
    └─ 解析并存储到 SQLite
  }
}
```

#### Step 2: 采集单个集群的指标
```rust
async fn collect_cluster_metrics(cluster: &Cluster) {
  1. 获取 Prometheus metrics 文本
  2. 解析 metrics -> metrics_map
  3. 聚合 Backend 指标 (CPU, 内存, 磁盘等)
  4. 聚合 Frontend 信息
  5. 聚合 Runtime 信息
  6. 保存 MetricsSnapshot 到 metrics_snapshots 表
}
```

#### Step 3: 数据清理
```
每个采集周期结束后 → 清理过期数据
├─ 删除 > 7 天的历史数据
└─ 删除 > 30 天的日聚合数据
```

#### Step 4: 日聚合 (Daily Aggregation)
```
每天一次 (UTC 00:00)
  ├─ 检查前一天的数据是否已聚合
  ├─ 若未聚合，运行日聚合任务
  └─ 生成 daily_snapshots
```

## Collected Metrics

### Query Performance
- QPS (Queries Per Second)
- RPS (Rows Per Second)
- Latency P50, P95, P99
- Query Count (total, success, error, timeout)

### Cluster Health
- Backend Count (total, alive)
- Frontend Count (total, alive)

### Resource Usage
- CPU Usage (total, average)
- Memory Usage (total, average)
- Disk Usage (bytes, percentage)
- JVM Heap Usage

### Storage Metrics
- Tablet Count
- Compaction Score

### Other
- Transaction Count (running, success, failed)
- Load Job Count (running, finished)
- Network Metrics (send/receive bytes and rates)
- IO Metrics (read/write bytes and rates)

## Data Storage

### Metrics Snapshots Table
```
metrics_snapshots {
  cluster_id: i64,
  collected_at: DateTime,
  qps: f64,
  rps: f64,
  // ... 90+ more fields
  backend_alive: i32,
  // ... etc
}
```

### Daily Aggregates Table
```
daily_snapshots {
  cluster_id: i64,
  snapshot_date: NaiveDate,
  avg_qps: f64,
  max_qps: f64,
  // ... aggregated metrics
}
```

## Configuration

### 配置文件位置
`backend/config/metrics-collector.toml`

### 可配置参数

```toml
[collector]
# 采集间隔（秒）- 默认 30秒
interval_secs = 30

# 数据保留天数 - 默认 7天
retention_days = 7

# 是否启用采集 - 默认 true
enabled = true
```

### 修改采集间隔
要修改采集间隔，编辑配置文件中的 `interval_secs` 值：

- **更频繁采集** (10秒): `interval_secs = 10`
- **较少采集** (60秒): `interval_secs = 60`
- **默认** (30秒): `interval_secs = 30`

## Frontend Usage

### 获取实时数据
前端可通过 GET `/api/clusters/overview` 获取最新采集的指标：

```typescript
this.overviewService.getExtendedClusterOverview(timeRange)
  ├─ 获取最新快照 (latest_snapshot)
  ├─ 获取历史趋势 (performance_trends, resource_trends)
  └─ 获取统计信息 (statistics)
```

### 前端自动刷新
- **集群概览页**: 支持自动刷新（15s, 30s, 1m）
- **其他页面**: 手动刷新只

## Performance Considerations

1. **采集频率**: 30秒是平衡实时性和性能的合理选择
2. **数据存储**: 7天保留期适合大多数监控场景
3. **并发安全**: 使用 tokio 的异步机制处理多集群采集
4. **错误处理**: 单个集群的采集失败不会影响其他集群

## Future Improvements

- [ ] 支持自定义采集指标列表
- [ ] 支持远程指标导出（Prometheus、Grafana）
- [ ] 支持告警规则配置
- [ ] 支持采集并发度配置
