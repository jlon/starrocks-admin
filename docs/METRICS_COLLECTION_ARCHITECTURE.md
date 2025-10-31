# Backend Metrics Collection Architecture

## Overview
后端通过定期采集机制从 StarRocks 集群实时收集性能与资源指标，并将快照写入 SQLite；同时按日聚合形成长期趋势数据。

- 采集调度：`ScheduledExecutor` 定期触发 `MetricsCollectorService::collect_once`
- 采集并发：单次周期内对每个集群并发请求 FE 的多个端点
- 保留策略：实时快照按天清理；日聚合按较长周期保留

## Collection Flow

### 1) 启动与配置
- 配置文件：`conf/config.toml`
- 配置项：
  - `metrics.interval_secs` (支持 "30s"/"5m"/"1h"/"1d")
  - `metrics.retention_days` (支持 7/"7d"/"2w")
  - `metrics.enabled`
- 环境变量覆盖：`APP_METRICS_INTERVAL_SECS`、`APP_METRICS_RETENTION_DAYS`、`APP_METRICS_ENABLED`
- main 读取配置后：若启用，则按 `interval_secs` 启动调度器，仅启动一个实例（单进程内单次 spawn）

### 2) 周期采集（每个 interval）
1. 遍历所有已注册集群（`ClusterService::list_clusters`）
2. 对每个集群执行 `collect_cluster_metrics`：
   - 并发请求（`tokio::try_join!`）：
     - `get_metrics()`：FE Prometheus Metrics 文本（HTTP）
     - `get_backends()`：BE 列表（SQL/HTTP 适配器，解析存活、Tablet 数量、CPU/MEM 等）
     - `get_frontends()`：FE 列表（角色/存活/端口/日志进度等）
     - `get_runtime_info()`：运行时信息（线程、堆、网络、IO 统计等）
   - 解析与聚合 → 写入 `metrics_snapshots`
3. 周期末执行：`cleanup_old_metrics()` 按保留天数清理历史快照
4. 每日一次：检查前一日是否已聚合，未聚合则运行日聚合生成 `daily_snapshots`

## Collected Metrics（采集项与来源）
> 字段见 `MetricsSnapshot`，这里只按类别汇总，并标注主要来源端点。

### Query Performance（主要来源：Prometheus metrics + Runtime）
- QPS、RPS
- 延迟：P50 / P95 / P99
- 查询计数：total / success / error / timeout

### Cluster Health（主要来源：Backends/Frontends 列表）
- Backend：total、alive
- Frontend：total、alive、角色（LEADER/FOLLOWER/OBSERVER）

### Resource Usage（主要来源：Backends + Prometheus metrics）
- CPU：total、avg
- Memory：total、avg
- Disk：total_bytes、used_bytes、usage_pct
- JVM：heap_total、heap_used、heap_usage_pct、thread_count

### Network & IO（主要来源：Prometheus metrics + Runtime）
- Network：bytes_sent_total、bytes_received_total、send_rate、receive_rate
- IO：read/write bytes total、read/write rate

### Storage & Compaction（主要来源：Backends）
- Tablet Count
- Max Compaction Score

### Transactions & Load（主要来源：Prometheus metrics/Runtime）
- Transactions：running、success_total、failed_total
- Load：running、finished_total

## Data Storage

### metrics_snapshots（实时快照）
- 每个周期写入一条（每集群），字段包含上述 90+ 指标
- 主字段：`cluster_id`、`collected_at` + 各类指标

### daily_snapshots（日聚合）
- 按日期聚合（avg / max 等），供趋势展示

## Risk Analysis（风险评估）

### FE/集群负载风险
- 请求端点：Prometheus `/metrics`（通常是文本较大、解析成本在后端）、`/api/backends`、`/api/frontends`、运行时信息端点
- 频率：默认 30s 对每个集群各请求一次；多集群时按集群数线性增加请求量
- 解析成本：主要在后端（Prometheus 文本解析、CPU/MEM 字符串解析）

结论：
- 在默认频率（30s）下，对 FE 的额外压力较低，主要流量为 `/metrics` 文本；多数生产 FE 足以承受。
- 若集群数量多（>20）或 FE 负载敏感，建议适当提高采集间隔（例如 60s/120s），或分批错峰（后续优化项）。

### 重复采集评估
- 单进程：仅在 `main` 中 spawn 一次调度器；无重复调度
- 多副本部署：若同时运行多个后端实例，会并行采集，可能造成重复入库
  - 目前未做去重。若要多副本部署，建议：
    - 方案A：仅一个实例启用 `metrics.enabled=true`，其余关闭
    - 方案B：引入基于数据库的分布式锁（后续优化项）

## Recommendations（优化建议）
- 配置层面：
  - 小规模/测试：`interval_secs = "30s"`
  - 中等规模：`interval_secs = "60s"` 或更长
  - 大规模/对 FE 敏感：`interval_secs = "120s"+`，并评估错峰
- 稳定性：
  - 设置 HTTP 请求超时（StarRocksClient 层），防止采集阻塞
  - 对 Prometheus 文本解析异常做降级与字段缺失容错（已有 warn 记录）
- 多副本：
  - 仅单实例启用采集或实现分布式去重/选主
- 存储保留：
  - `retention_days` 根据磁盘容量与可视化需求调整（默认 7 天）

## Configuration（配置）

### 路径
- `conf/config.toml`

### 示例
```toml
[metrics]
# 采集间隔（支持 30 / "30s" / "5m" / "1h" / "1d"）
interval_secs = "30s"
# 数据保留（支持 7 / "7d" / "2w"）
retention_days = "7d"
# 是否启用采集
enabled = true
```

### 环境变量覆盖
```bash
APP_METRICS_INTERVAL_SECS=1m \
APP_METRICS_RETENTION_DAYS=14d \
APP_METRICS_ENABLED=true \
./bin/starrocks-admin.sh start
```

## Frontend Usage（前端使用）
- GET `/api/clusters/overview` 获取：
  - `latest_snapshot`（最新实时快照）
  - `performance_trends` / `resource_trends`（历史趋势）
  - `statistics`（聚合统计）
- 自动刷新仅保留在“集群概览”页（15s/30s/1m，可手动切换）

## Future Improvements
- 请求错峰与最大并发限制（避免大规模集群瞬时压测 FE）
- 多实例采集选主/分布式锁，避免重复采集
- 指标集可配置、采样降频、采集失败自适应退避
