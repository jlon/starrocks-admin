# 库表右键菜单功能 - SQL查询完整列表

本文档列出了当前实现的所有库表右键菜单功能的SQL查询语句。

## 前端开发规范

### ⚠️ 核心原则

**本项目前端基于 `/home/oppo/Documents/ngx-admin` 开发，必须严格遵循以下规范：**

1. **优先使用 ngx-admin/Nebular 原生组件和样式**
2. **不轻易自定义**：只有在原生组件无法满足需求时才自定义
3. **自定义必须兼容**：如需自定义，必须使用 ngx-admin 原生样式或风格兼容的样式
4. **重要**：不轻易自定义 ≠ 不能自定义！当确实需要自定义时，必须保持与 ngx-admin 风格完全一致

---

### 组件使用规范

#### 1. 必须优先使用的原生组件

- **对话框**：`nb-dialog`, `nb-card`, `nb-card-header`, `nb-card-body`, `nb-card-footer`
- **Tab切换**：`nb-tabset`, `nb-tab`
- **按钮**：`nb-button`（支持 `status`, `size`, `ghost` 等原生属性）
- **选择器**：`nb-select`, `nb-option`
- **表格**：`ng2-smart-table`（Nebular 官方推荐）
- **图标**：`nb-icon`（使用 Eva Icons）
- **加载器**：`nb-spinner`（支持 `status`, `size` 等原生属性）
- **布局**：`nb-layout`, `nb-sidebar`, `nb-layout-column` 等

#### 2. 原生样式类（必须优先使用）

**Badge样式**（状态标识）：
- `badge-success` - 成功/完成状态
- `badge-danger` - 错误/危险状态
- `badge-info` - 信息/进行中状态
- `badge-warning` - 警告/等待状态
- `badge-basic` - 基础/默认状态

**文本样式**：
- `text-hint` - 提示文本
- `text-primary` - 主要文本
- `text-success` - 成功文本
- `text-danger` - 错误文本

**布局类**：
- Bootstrap 网格系统（`row`, `col-*`）
- Nebular 布局系统（`nb-layout`, `nb-sidebar` 等）

---

### 样式自定义规范

#### 1. 自定义样式的前提条件

✅ **可以自定义的情况**：
- 原生组件无法满足功能需求
- 需要特殊的布局或交互效果
- 原生样式类无法实现所需效果

❌ **禁止自定义的情况**：
- 原生组件已提供相同功能
- 仅为了改变外观而自定义（应使用主题系统）
- 自定义样式与 ngx-admin 风格不一致

#### 2. 自定义样式的实现方式

**必须使用 Nebular 主题系统**：

```scss
// ✅ 正确：使用 nb-theme() 函数
.custom-component {
  background-color: nb-theme(background-basic-color-2);
  border: 1px solid nb-theme(border-basic-color-3);
  color: nb-theme(text-basic-color);
  border-radius: nb-theme(border-radius);
  
  &:hover {
    background-color: nb-theme(background-basic-color-3);
  }
}

// ❌ 错误：硬编码颜色值
.custom-component {
  background-color: #f5f5f5;
  border: 1px solid #ddd;
  color: #333;
}
```

**必须使用 Nebular 主题变量**：
- `nb-theme(background-basic-color-*)` - 背景色
- `nb-theme(text-basic-color)` - 文本色
- `nb-theme(border-basic-color-*)` - 边框色
- `nb-theme(border-radius)` - 圆角
- `nb-theme(shadow-*)` - 阴影
- 等等...

#### 3. 自定义样式的风格要求

- **颜色**：必须使用 Nebular 主题变量，支持深色/浅色主题切换
- **间距**：遵循 Nebular 的间距规范（`0.5rem`, `1rem`, `1.5rem` 等）
- **圆角**：使用 `nb-theme(border-radius)`
- **阴影**：使用 `nb-theme(shadow-*)`
- **过渡效果**：使用 `transition: all 0.2s ease` 等标准过渡

---

### 当前实现遵循情况

#### ✅ 已正确使用原生组件

- ✅ 事务查询使用 `nb-tabset` 原生组件
- ✅ 对话框使用 `nb-dialog` 和 `nb-card` 原生组件
- ✅ 表格使用 `ng2-smart-table` 组件
- ✅ 状态显示使用原生 `badge` 样式类（`badge-success`, `badge-danger`, `badge-info`, `badge-warning`）
- ✅ 分页选择器使用 `nb-select` 原生组件
- ✅ 按钮使用 `nb-button` 原生组件
- ✅ 图标使用 `nb-icon` 原生组件
- ✅ 加载器使用 `nb-spinner` 原生组件

#### ✅ 自定义样式符合规范

- ✅ 使用 `nb-theme()` 函数访问主题变量
- ✅ 自定义样式与 ngx-admin 风格兼容
- ✅ 支持深色/浅色主题切换

---

### 开发检查清单

在添加新功能或修改现有功能时，请检查：

- [ ] 是否优先尝试使用原生组件？
- [ ] 如果自定义样式，是否使用了 `nb-theme()` 函数？
- [ ] 自定义样式是否与 ngx-admin 风格一致？
- [ ] 是否支持主题切换（深色/浅色）？
- [ ] Badge 是否使用了原生样式类？
- [ ] 按钮是否使用了 `nb-button` 组件？
- [ ] 图标是否使用了 `nb-icon` 组件？

---

### 参考资源

- [ngx-admin GitHub](https://github.com/akveo/ngx-admin)
- [Nebular 文档](https://akveo.github.io/nebular/)
- [Nebular 主题系统](https://akveo.github.io/nebular/docs/design-system/theme-system)
- [Eva Icons](https://akveo.github.io/eva-icons/)

## 数据库级别右键菜单

### 1. 查看事务信息 (viewDatabaseTransactions)

**实现方式**（已修复 ✅）：

1. 首先获取数据库ID：
```sql
SHOW PROC '/dbs'
```

2. 查询运行中事务：
```sql
SHOW PROC '/transactions/<db_id>/running'
```

3. 查询已完成事务：
```sql
SHOW PROC '/transactions/<db_id>/finished'
```

**返回字段**：
- TransactionId: 事务ID
- Label: 标签
- Coordinator: 协调者
- TransactionStatus: 事务状态（VISIBLE, ABORTED, COMMITTED等）
- LoadJobSourceType: 来源类型
- PrepareTime: 准备时间
- CommitTime: 提交时间
- PublishTime: 发布时间
- FinishTime: 完成时间
- ErrMsg: 错误信息

**UI实现**：
- 使用 ngx-admin 原生的 `nb-tabset` 组件实现tab切换
- 两个tab：运行中 / 已完成
- 表级别查询支持按表名过滤（通过Label字段）

**状态**：✅ 已实现并修复

---

### 2. 查看Compaction信息 (viewDatabaseCompactions)

**实现方式**（已修复 ✅）：

查询Compaction任务详情：
```sql
SHOW PROC '/compactions'
```

**返回字段**：
- Partition: 分区信息（格式：`database.table.partition_id`）
- TxnID: 事务ID
- StartTime: 开始时间
- CommitTime: 提交时间（NULL表示未提交）
- FinishTime: 完成时间（NULL表示进行中）
- Error: 错误信息
- Profile: JSON格式的性能信息（包含子任务数、读取数据量等）

**数据处理**：
- 自动解析Partition字段，显示数据库、表名和分区ID
- 数据库级别：自动过滤该数据库的Compaction任务（通过Partition字段前缀匹配）
- 显示任务状态：进行中（FinishTime为NULL）或已完成
- Profile信息格式化显示：子任务数、本地读取、远程读取等

**UI实现**：
- 使用 ngx-admin 原生的 `ng2-smart-table` 组件显示数据
- 使用原生 `nb-badge` 样式显示状态（进行中/已完成）
- 错误信息使用长文本截断工具函数显示

**状态**：✅ 已实现并修复

---

### 3. 查看导入作业 (viewDatabaseLoads)

```sql
SELECT 
  JOB_ID,
  LABEL,
  STATE,
  PROGRESS,
  TYPE,
  PRIORITY,
  SCAN_ROWS,
  FILTERED_ROWS,
  SINK_ROWS,
  CREATE_TIME,
  LOAD_START_TIME,
  LOAD_FINISH_TIME,
  ERROR_MSG
FROM information_schema.loads 
WHERE DATABASE_NAME = '<database_name>'
ORDER BY CREATE_TIME DESC
LIMIT 100
```

**返回字段**：
- JOB_ID: 作业ID
- LABEL: 标签
- STATE: 状态（FINISHED, LOADING, PENDING, CANCELLED, QUEUEING）
- PROGRESS: 进度
- TYPE: 类型
- PRIORITY: 优先级
- SCAN_ROWS: 扫描行数
- FILTERED_ROWS: 过滤行数
- SINK_ROWS: 导入行数
- CREATE_TIME: 创建时间
- LOAD_START_TIME: 开始时间
- LOAD_FINISH_TIME: 完成时间
- ERROR_MSG: 错误信息

**状态**：✅ 实现正常

---

### 4. 查看数据库统计 (viewDatabaseStats)

**实现方式**（已修复 ✅）：

```sql
SELECT 
  TABLE_NAME,
  COUNT(DISTINCT PARTITION_NAME) as PARTITION_COUNT,
  SUM(ROW_COUNT) as TOTAL_ROWS,
  SUM(CASE WHEN DATA_SIZE LIKE '%KB' THEN CAST(REPLACE(DATA_SIZE, 'KB', '') AS DECIMAL) / 1024
           WHEN DATA_SIZE LIKE '%MB' THEN CAST(REPLACE(DATA_SIZE, 'MB', '') AS DECIMAL)
           WHEN DATA_SIZE LIKE '%GB' THEN CAST(REPLACE(DATA_SIZE, 'GB', '') AS DECIMAL) * 1024
           ELSE 0 END) as TOTAL_SIZE_MB,
  AVG(MAX_CS) as AVG_MAX_CS,
  MAX(MAX_CS) as MAX_CS_OVERALL
FROM information_schema.partitions_meta 
WHERE DB_NAME = '<database_name>'
GROUP BY TABLE_NAME
ORDER BY TOTAL_ROWS DESC
```

**返回字段**：
- TABLE_NAME: 表名
- PARTITION_COUNT: 分区数
- TOTAL_ROWS: 总行数
- TOTAL_SIZE_MB: 总大小(MB)
- AVG_MAX_CS: 平均最大CS（已修复，直接使用MAX_CS字段聚合）
- MAX_CS_OVERALL: 最大CS（已修复，直接使用MAX_CS字段聚合）

**修复说明**：
- ✅ CS分数查询已修复，直接使用 `MAX_CS` 字段进行聚合计算

**UI实现**：
- 使用 ngx-admin 原生的 `ng2-smart-table` 组件
- CS分数使用自定义渲染函数显示，带有颜色标识

**状态**：✅ 已实现并修复

---

## 表级别右键菜单

### 1. 查看表结构 (viewTableSchema)

```sql
SHOW CREATE TABLE `<catalog>`.`<database>`.`<table>`
```

**返回字段**：
- Create Table: 建表语句

**状态**：✅ 实现正常

---

### 1.1. 查看视图查询计划 (viewViewQueryPlan) - 仅视图

**实现方式**（新增 ✅）：

```sql
EXPLAIN SELECT * FROM `<catalog>`.`<database>`.`<view>` LIMIT 1
```

**返回字段**：
- Explain String: 查询计划文本（多行）

**功能说明**：
- 显示视图的查询执行计划
- 帮助分析视图查询性能
- 快速定位视图查询慢的原因

**UI实现**：
- 使用与表结构相同的对话框显示
- 显示格式化的查询计划文本

**适用对象**：
- 仅适用于 VIEW 类型
- **注意**：物化视图（MATERIALIZED_VIEW）不支持此功能，因为物化视图是物理表，`EXPLAIN SELECT * FROM <mv>` 只是简单的表扫描，意义不大。如需了解物化视图的构建逻辑，请查看物化视图的定义（CREATE MATERIALIZED VIEW 语句）。

**状态**：✅ 已实现

---

### 2. 查看分区 (viewTablePartitions)

**实现方式**（已修复 ✅）：

```sql
SELECT 
  PARTITION_NAME,
  PARTITION_ID,
  PARTITION_KEY,
  PARTITION_VALUE,
  DATA_SIZE,
  ROW_COUNT,
  AVG_CS,
  P50_CS,
  MAX_CS,
  COMPACT_VERSION,
  VISIBLE_VERSION,
  STORAGE_PATH
FROM information_schema.partitions_meta 
WHERE DB_NAME = '<database_name>' AND TABLE_NAME = '<table_name>'
ORDER BY PARTITION_NAME
```

**返回字段**：
- PARTITION_NAME: 分区名
- PARTITION_ID: 分区ID
- PARTITION_KEY: 分区键（长文本，已应用缩略）
- PARTITION_VALUE: 分区值（长文本，已应用缩略）
- DATA_SIZE: 数据大小
- ROW_COUNT: 行数
- AVG_CS: 平均CS（已修复，直接使用字段名）
- P50_CS: P50 CS（已修复，直接使用字段名）
- MAX_CS: 最大CS（已修复，直接使用字段名）
- COMPACT_VERSION: Compact版本
- VISIBLE_VERSION: 可见版本
- STORAGE_PATH: 存储路径（长文本，已应用缩略）

**修复说明**：
- ✅ CS分数查询已修复，直接使用 `AVG_CS`, `P50_CS`, `MAX_CS` 字段

**UI实现**：
- 使用 ngx-admin 原生的 `ng2-smart-table` 组件
- 长文本字段（PARTITION_KEY, PARTITION_VALUE, STORAGE_PATH）使用工具函数进行截断显示
- CS分数使用自定义渲染函数显示，带有颜色标识

**限制**：
- 视图（VIEW）不支持此功能

**状态**：✅ 已实现并修复

---

### 3. 查看Compaction Score (viewTableCompactionScore)

**实现方式**（已修复 ✅）：

```sql
SELECT 
  PARTITION_NAME,
  AVG_CS,
  P50_CS,
  MAX_CS,
  DATA_SIZE,
  ROW_COUNT,
  COMPACT_VERSION,
  VISIBLE_VERSION
FROM information_schema.partitions_meta 
WHERE DB_NAME = '<database_name>' AND TABLE_NAME = '<table_name>'
ORDER BY MAX_CS DESC
```

**返回字段**：
- PARTITION_NAME: 分区名
- AVG_CS: 平均CS（已修复，直接使用字段名）
- P50_CS: P50 CS（已修复，直接使用字段名）
- MAX_CS: 最大CS（已修复，直接使用字段名）
- DATA_SIZE: 数据大小
- ROW_COUNT: 行数
- COMPACT_VERSION: Compact版本
- VISIBLE_VERSION: 可见版本

**修复说明**：
- ✅ 移除了不必要的COALESCE处理
- ✅ 直接使用 `information_schema.partitions_meta` 表的 `AVG_CS`, `P50_CS`, `MAX_CS` 字段
- ✅ 测试确认字段名正确，数据正常显示

**UI实现**：
- 使用 ngx-admin 原生的 `ng2-smart-table` 组件
- CS分数使用自定义渲染函数显示，带有颜色标识（基于阈值）

**限制**：
- 视图（VIEW）不支持此功能

**状态**：✅ 已实现并修复

---

### 4. 查看表统计 (viewTableStats)

**实现方式**（已修复 ✅）：

```sql
SELECT 
  PARTITION_NAME,
  PARTITION_ID,
  DATA_SIZE,
  ROW_COUNT,
  BUCKETS,
  REPLICATION_NUM,
  STORAGE_MEDIUM,
  AVG_CS,
  MAX_CS,
  STORAGE_PATH
FROM information_schema.partitions_meta 
WHERE DB_NAME = '<database_name>' AND TABLE_NAME = '<table_name>'
ORDER BY PARTITION_NAME
```

**返回字段**：
- PARTITION_NAME: 分区名
- PARTITION_ID: 分区ID
- DATA_SIZE: 数据大小
- ROW_COUNT: 行数
- BUCKETS: 分桶数
- REPLICATION_NUM: 副本数
- STORAGE_MEDIUM: 存储介质
- AVG_CS: 平均CS（已修复，直接使用字段名）
- MAX_CS: 最大CS（已修复，直接使用字段名）
- STORAGE_PATH: 存储路径（长文本，已应用缩略）

**修复说明**：
- ✅ CS分数查询已修复，直接使用 `AVG_CS`, `MAX_CS` 字段

**UI实现**：
- 使用 ngx-admin 原生的 `ng2-smart-table` 组件
- 长文本字段使用工具函数进行截断显示

**状态**：✅ 已实现并修复

---

### 5. 查看表事务 (viewTableTransactions)

**实现方式**（已修复 ✅）：

与数据库级别事务查询相同，但增加了表名过滤：

1. 获取数据库ID：
```sql
SHOW PROC '/dbs'
```

2. 查询运行中事务：
```sql
SHOW PROC '/transactions/<db_id>/running'
```

3. 查询已完成事务：
```sql
SHOW PROC '/transactions/<db_id>/finished'
```

4. 前端过滤：根据Label字段包含表名进行过滤

**返回字段**：同数据库级别事务信息

**UI实现**：
- 使用 ngx-admin 原生的 `nb-tabset` 组件实现tab切换
- 两个tab：运行中 / 已完成
- 自动过滤该表相关的事务

**限制**：
- 视图（VIEW）不支持此功能

**状态**：✅ 已实现并修复

---

### 6. 手动触发Compaction (triggerCompaction)

**触发整个表的Compaction**：
```sql
ALTER TABLE `<catalog>`.`<database>`.`<table>` COMPACT
```

**触发单个分区的Compaction**：
```sql
ALTER TABLE `<catalog>`.`<database>`.`<table>` COMPACT `<partition_name>`
```

**触发多个分区的Compaction**：
```sql
ALTER TABLE `<catalog>`.`<database>`.`<table>` COMPACT (`<partition1>`, `<partition2>`, ...)
```

**获取可用分区列表**：
```sql
SELECT PARTITION_NAME 
FROM information_schema.partitions_meta 
WHERE DB_NAME = '<database_name>' AND TABLE_NAME = '<table_name>'
ORDER BY PARTITION_NAME
```

**状态**：✅ 实现正常

**限制**：
- 视图（VIEW）不支持
- 物化视图（MATERIALIZED_VIEW）不建议手动触发

---

### 7. 查看物化视图刷新状态 (viewMaterializedViewRefreshStatus) - 仅物化视图

**实现方式**（新增 ✅）：

```sql
SELECT 
  TABLE_NAME,
  IS_ACTIVE,
  REFRESH_TYPE,
  LAST_REFRESH_STATE,
  LAST_REFRESH_START_TIME,
  LAST_REFRESH_FINISHED_TIME,
  LAST_REFRESH_DURATION,
  LAST_REFRESH_ERROR_MESSAGE,
  INACTIVE_REASON
FROM information_schema.materialized_views 
WHERE TABLE_SCHEMA = '<database_name>' AND TABLE_NAME = '<mv_name>'
```

**返回字段**：
- TABLE_NAME: 物化视图名
- IS_ACTIVE: 是否激活（true/false）
- REFRESH_TYPE: 刷新类型（ASYNC/ROLLUP）
- LAST_REFRESH_STATE: 最后刷新状态（SUCCESS/FAILED/RUNNING等）
- LAST_REFRESH_START_TIME: 最后刷新开始时间
- LAST_REFRESH_FINISHED_TIME: 最后刷新完成时间
- LAST_REFRESH_DURATION: 最后刷新耗时（秒）
- LAST_REFRESH_ERROR_MESSAGE: 最后刷新错误信息
- INACTIVE_REASON: 未激活原因

**功能说明**：
- 快速查看物化视图的刷新状态
- 帮助定位物化视图刷新失败问题
- 查看刷新耗时和错误信息

**UI实现**：
- 使用 ngx-admin 原生的 `ng2-smart-table` 组件
- 状态字段使用原生 `badge` 样式显示（成功/失败/进行中等）
- 错误信息使用长文本截断工具函数显示

**适用对象**：
- 仅适用于 MATERIALIZED_VIEW 类型

**状态**：✅ 已实现

---

## 已实现的功能详情

### 1. SHOW PROC '/compactions' - 查看Compaction任务 ✅

**实现状态**：已实现

**SQL查询**：
```sql
SHOW PROC '/compactions'
```

**返回字段**（实际实现）：
- Partition: 分区信息（格式：`database.table.partition_id`）
- TxnID: 事务ID
- StartTime: 开始时间
- CommitTime: 提交时间
- FinishTime: 完成时间
- Error: 错误信息
- Profile: JSON格式的性能信息

**实现细节**：
- 数据库级别：自动过滤该数据库的Compaction任务（通过Partition字段前缀匹配）
- 表级别：可通过Partition字段解析过滤（待实现）
- UI：使用 `ng2-smart-table` 显示，状态使用原生badge样式

**取消Compaction任务**（待实现）：
```sql
CANCEL COMPACTION WHERE TXN_ID = <TXN_ID>
```

---

### 2. SHOW TRANSACTION - 查看事务 ✅

**实现状态**：已实现

**SQL查询**：
```sql
-- 1. 获取数据库ID
SHOW PROC '/dbs'

-- 2. 查询运行中事务
SHOW PROC '/transactions/<db_id>/running'

-- 3. 查询已完成事务
SHOW PROC '/transactions/<db_id>/finished'
```

**实现细节**：
- ✅ 数据库级别：显示两个tab（运行中/已完成）
- ✅ 表级别：显示该表相关的事务（通过Label字段过滤）
- ✅ UI：使用 `nb-tabset` 原生组件实现tab切换
- ✅ 状态显示：使用原生badge样式（`badge-success`, `badge-danger` 等）

---

## 总结

### 已实现的功能 ✅
1. 查看表结构（SHOW CREATE TABLE）
2. 查看分区信息
3. 查看导入作业
4. 查看数据库统计
5. 查看表统计
6. 手动触发Compaction
7. 分页每页条数选择器
8. **查看Compaction任务** - 使用 `SHOW PROC '/compactions'`（已修复）
9. **查看事务信息** - 使用 `SHOW PROC '/transactions/<db_id>/running'` 和 `/finished'`，支持tab切换（已修复）
10. **CS分数查询** - 直接使用 `AVG_CS`, `P50_CS`, `MAX_CS` 字段（已修复）
11. **查看物化视图刷新状态** - 使用 `information_schema.materialized_views`（新增）
12. **查看视图查询计划** - 使用 `EXPLAIN SELECT * FROM <view> LIMIT 1`，仅支持 VIEW（新增，物化视图不支持，因为物化视图是物理表，查询计划只是表扫描）

### 已修复的问题 ✅
1. **CS分数显示为0**：
   - ✅ 已修复：移除了不必要的COALESCE，直接使用 `information_schema.partitions_meta` 表的 `AVG_CS`, `P50_CS`, `MAX_CS` 字段
   - 测试确认：字段名正确，数据正常显示

2. **事务查询**：
   - ✅ 已修复：使用 `SHOW PROC '/dbs'` 获取数据库ID，然后使用 `SHOW PROC '/transactions/<db_id>/running'` 和 `/finished'` 查询
   - ✅ 已实现：在对话框中添加tab切换（运行中/已完成）
   - ✅ 已实现：表级别事务查询支持按表名过滤

3. **Compaction任务查询**：
   - ✅ 已实现：使用 `SHOW PROC '/compactions'` 查看Compaction任务详情
   - ✅ 已实现：数据库级别自动过滤该数据库的Compaction任务
   - ✅ 已实现：解析Partition字段显示数据库、表名和分区ID
   - ✅ 已实现：显示任务状态（进行中/已完成）、错误信息和Profile信息

### 未实现的功能 ❌
1. **取消Compaction任务** - CANCEL COMPACTION（可通过SQL手动执行）

---

## 建议的改进方案

### 已实施的修复方案

#### 1. CS分数查询修复 ✅
```sql
-- 直接使用information_schema.partitions_meta表的字段
SELECT 
  TABLE_NAME,
  PARTITION_NAME,
  AVG_CS,
  P50_CS,
  MAX_CS,
  ...
FROM information_schema.partitions_meta 
WHERE DB_NAME = '<database_name>'
ORDER BY MAX_CS DESC
```
- 移除了COALESCE处理，直接使用正确的字段名
- 测试确认字段名和数据格式正确

#### 2. Compaction任务查询实现 ✅
```sql
-- 查询所有Compaction任务
SHOW PROC '/compactions'
```
- 返回字段：Partition, TxnID, StartTime, CommitTime, FinishTime, Error, Profile
- Partition格式：`database.table.partition_id`
- 数据库级别：自动过滤该数据库的任务
- 表级别：可通过Partition字段解析过滤

#### 3. 事务查询改进 ✅
```sql
-- 1. 获取数据库ID
SHOW PROC '/dbs'

-- 2. 查询运行中事务
SHOW PROC '/transactions/<db_id>/running'

-- 3. 查询已完成事务
SHOW PROC '/transactions/<db_id>/finished'
```
- 在对话框中添加tab切换（运行中/已完成）
- 表级别事务查询支持按表名过滤（通过Label字段）

---

## 测试建议

### 已完成的测试 ✅

1. **CS分数查询测试**：
   - ✅ 测试确认 `information_schema.partitions_meta` 表字段名为 `AVG_CS`, `P50_CS`, `MAX_CS`
   - ✅ 测试确认数据正常显示，不再为0

2. **SHOW PROC支持测试**：
   - ✅ 测试确认后端支持 `SHOW PROC '/compactions'`
   - ✅ 测试确认返回的数据格式正确
   - ✅ 测试确认 `SHOW PROC '/dbs'` 返回数据库ID映射
   - ✅ 测试确认 `SHOW PROC '/transactions/<db_id>/running'` 和 `/finished'` 可用

3. **事务查询测试**：
   - ✅ 测试确认通过数据库ID查询事务的方法有效
   - ✅ 测试确认tab切换功能正常

### 测试集群信息

- 集群名称：easychange-starrocks
- 地址：10.212.189.201
- 账户：root / hj$joS)iM2jG
- 端口：9030（默认）
- 测试数据库：yh_test

### 待测试功能

1. **表级别Compaction任务过滤**：
   - 需要实现按表名过滤Compaction任务列表
   - 可通过解析Partition字段实现

2. **取消Compaction任务**：
   - 需要实现 `CANCEL COMPACTION WHERE TXN_ID = <TXN_ID>` 功能
   - 可在Compaction任务列表中添加操作按钮

