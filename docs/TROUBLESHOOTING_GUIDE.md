# 事务爆炸和Compaction问题快速定位指南

本文档说明如何通过库表树上的右键菜单快速定位事务爆炸和Compaction相关问题。

## 问题场景

**场景1：数据库事务爆炸，达到上限**
- 现象：数据库事务数量过多，可能达到系统上限
- 需要定位：哪个表、哪个分区有问题
- 需要判断：是否是Compaction问题导致的

## 快速定位动线

### 第一步：数据库级别 - 查看事务信息

**操作路径**：数据库节点右键 → `查看事务信息`

**功能说明**：
- 查看运行中事务（Running Tab）：显示当前正在执行的事务
- 查看已完成事务（Finished Tab）：显示最近完成的事务

**关键指标**：
1. **运行中事务数量**：如果数量过多（接近上限），说明有大量事务未完成
2. **事务状态**：
   - `VISIBLE`：已提交并可见
   - `COMMITTED`：已提交但未发布
   - `ABORTED`：已中止
   - 长时间处于 `COMMITTED` 状态的事务可能是Compaction问题
3. **事务持续时间**：查看 `PrepareTime` 和 `CommitTime`，如果事务长时间未完成，可能是Compaction阻塞

**判断标准**：
- 如果运行中事务数量 > 100，需要进一步排查
- 如果事务长时间处于 `COMMITTED` 状态，可能是Compaction未及时完成
- 查看 `Label` 字段，可以识别是哪个表的事务

---

### 第二步：数据库级别 - 查看Compaction信息

**操作路径**：数据库节点右键 → `查看Compaction信息`

**功能说明**：
- 显示该数据库的所有Compaction任务
- 包括正在进行的和已完成的Compaction任务

**关键指标**：
1. **Compaction任务状态**：
   - `FinishTime` 为 `NULL`：任务正在进行中
   - `FinishTime` 有值：任务已完成
   - `Error` 有值：任务失败
2. **Compaction任务数量**：如果大量任务堆积，说明Compaction跟不上数据写入速度
3. **Partition字段**：格式为 `database.table.partition_id`，可以识别是哪个表、哪个分区
4. **Profile信息**：包含子任务数、读取数据量等性能指标

**判断标准**：
- 如果大量Compaction任务处于进行中状态，且持续时间很长，说明Compaction性能不足
- 如果Compaction任务频繁失败（Error字段有值），需要检查错误信息
- 如果某个表/分区的Compaction任务特别多，说明该表/分区有问题

---

### 第三步：数据库级别 - 查看数据库统计

**操作路径**：数据库节点右键 → `查看数据库统计`

**功能说明**：
- 显示数据库中所有表的统计信息
- 包括：表名、分区数、总行数、总大小、平均最大CS、最大CS

**关键指标**：
1. **MAX_CS_OVERALL（最大CS）**：
   - CS（Compaction Score）表示分区需要Compaction的紧急程度
   - CS越高，表示该表/分区越需要Compaction
   - **判断标准**：CS > 100 表示需要紧急Compaction
2. **AVG_MAX_CS（平均最大CS）**：
   - 表示该表所有分区的平均CS
   - 如果平均值很高，说明该表整体需要Compaction
3. **分区数（PARTITION_COUNT）**：
   - 分区数越多，Compaction任务越多
   - 如果某个表分区数特别多，且CS很高，可能是问题表

**判断标准**：
- 按 `MAX_CS_OVERALL` 降序排序，找出CS最高的表
- 如果某个表的 `MAX_CS_OVERALL` > 100，需要重点关注
- 如果某个表的 `AVG_MAX_CS` > 50，说明该表整体需要Compaction

---

### 第四步：表级别 - 查看Compaction Score

**操作路径**：表节点右键 → `查看Compaction Score`

**功能说明**：
- 显示该表所有分区的Compaction Score
- 按 `MAX_CS` 降序排序，优先显示CS最高的分区

**关键指标**：
1. **MAX_CS（最大CS）**：
   - 单个分区的最大CS
   - **判断标准**：MAX_CS > 100 表示该分区需要紧急Compaction
2. **AVG_CS（平均CS）**：
   - 该分区的平均CS
3. **P50_CS（P50 CS）**：
   - 该分区的P50 CS
4. **COMPACT_VERSION vs VISIBLE_VERSION**：
   - 如果两者差距很大，说明Compaction未及时完成
   - 差距 = VISIBLE_VERSION - COMPACT_VERSION

**判断标准**：
- 找出 `MAX_CS` 最高的分区
- 如果 `MAX_CS` > 100，该分区需要立即Compaction
- 如果 `COMPACT_VERSION` 和 `VISIBLE_VERSION` 差距 > 10，说明Compaction滞后

---

### 第五步：表级别 - 查看分区

**操作路径**：表节点右键 → `查看分区`

**功能说明**：
- 显示该表所有分区的详细信息
- 包括：分区名、分区ID、分区键、分区值、数据大小、行数、CS分数等

**关键指标**：
1. **MAX_CS**：分区的最大CS（同上）
2. **ROW_COUNT**：分区行数，如果行数特别多，Compaction可能较慢
3. **DATA_SIZE**：分区数据大小，如果数据量特别大，Compaction可能较慢
4. **COMPACT_VERSION vs VISIBLE_VERSION**：版本差距

**判断标准**：
- 找出CS最高、数据量最大的分区
- 这些分区可能是导致事务爆炸的原因

---

### 第六步：表级别 - 查看表事务

**操作路径**：表节点右键 → `查看表事务`

**功能说明**：
- 显示该表相关的事务（通过Label字段过滤）
- 包括运行中和已完成的事务

**关键指标**：
1. **事务数量**：该表的事务数量
2. **事务状态**：是否有大量事务未完成
3. **事务持续时间**：事务是否长时间未完成

**判断标准**：
- 如果该表有大量运行中事务，说明该表可能是问题源头
- 如果事务长时间未完成，可能是Compaction阻塞

---

### 第七步：手动触发Compaction（可选）

**操作路径**：表节点右键 → `手动触发Compaction`

**功能说明**：
- 手动触发整个表或指定分区的Compaction
- 可以立即开始Compaction，不需要等待系统自动调度

**使用场景**：
- 当发现某个表/分区的CS特别高时
- 当Compaction任务堆积时
- 当需要立即清理事务时

**注意事项**：
- 手动触发Compaction会消耗系统资源
- 建议在业务低峰期执行
- 可以先触发CS最高的分区

---

## 问题诊断流程

### 场景：数据库事务爆炸

**诊断步骤**：

1. **数据库右键 → 查看事务信息**
   - 查看运行中事务数量
   - 如果数量 > 100，继续下一步

2. **数据库右键 → 查看Compaction信息**
   - 查看Compaction任务状态
   - 如果大量任务进行中或失败，继续下一步

3. **数据库右键 → 查看数据库统计**
   - 按 `MAX_CS_OVERALL` 降序排序
   - 找出CS最高的表（CS > 100）

4. **表右键 → 查看Compaction Score**
   - 查看该表所有分区的CS
   - 找出CS最高的分区（CS > 100）

5. **表右键 → 查看分区**
   - 查看该分区的详细信息
   - 检查 `COMPACT_VERSION` 和 `VISIBLE_VERSION` 差距

6. **表右键 → 查看表事务**
   - 查看该表的事务情况
   - 确认是否有大量事务未完成

7. **判断是否为Compaction问题**：
   - ✅ 如果CS很高（> 100），且Compaction任务堆积 → **是Compaction问题**
   - ✅ 如果CS很高，但Compaction任务正常 → **可能是Compaction性能不足**
   - ✅ 如果CS正常，但事务数量多 → **可能是其他问题（如导入频率过高）**

8. **解决方案**：
   - 如果是Compaction问题：
     - 手动触发CS最高的分区Compaction
     - 检查Compaction配置参数（`compact_threads`, `max_cumulative_compaction_num_singleton_deltas`）
     - 考虑增加Compaction资源
   - 如果是其他问题：
     - 检查导入频率
     - 检查事务超时配置
     - 考虑增加事务上限

---

## 关键指标参考

### Compaction Score (CS) 阈值

根据 [StarRocks Compaction文档](https://forum.mirrorship.cn/t/topic/13256)：

- **CS < 10**：正常，不需要Compaction
- **10 ≤ CS < 50**：需要Compaction，但不紧急
- **50 ≤ CS < 100**：需要Compaction，建议尽快处理
- **CS ≥ 100**：紧急，需要立即Compaction

### 事务数量阈值

- **运行中事务 < 50**：正常
- **50 ≤ 运行中事务 < 100**：需要关注
- **运行中事务 ≥ 100**：可能达到上限，需要排查

### Compaction任务状态

- **进行中任务 < 10**：正常
- **10 ≤ 进行中任务 < 20**：需要关注
- **进行中任务 ≥ 20**：可能Compaction性能不足

---

## 最佳实践

1. **定期检查**：
   - 每天检查一次数据库统计，关注CS最高的表
   - 每周检查一次Compaction任务状态

2. **设置告警**：
   - CS > 100 时告警
   - 运行中事务 > 100 时告警
   - Compaction任务失败时告警

3. **优化Compaction**：
   - 根据实际情况调整 `compact_threads` 参数
   - 调整 `max_cumulative_compaction_num_singleton_deltas` 参数（建议100）
   - 在业务低峰期手动触发高CS分区的Compaction

4. **监控关键指标**：
   - 数据库统计中的 `MAX_CS_OVERALL`
   - Compaction信息中的任务状态
   - 事务信息中的运行中事务数量

---

## 相关文档

- [StarRocks Compaction原理与调优指南](https://forum.mirrorship.cn/t/topic/13256)
- [StarRocks官方文档 - Compaction管理](https://docs.starrocks.io/zh/docs/3.5/administration/management/compaction/)
- [库表右键菜单功能文档](./RIGHT_CLICK_MENU_SQL_QUERIES.md)

