# StarRocks Profile Metrics Reference

> 本文档基于 StarRocks 源码分析，系统性记录 Profile 中所有指标的定义、含义和层级关系。

## 1. Profile 架构概述

```
Query Profile
├── Summary (Query 级别)
│   ├── Query ID, Start Time, End Time, Total Time
│   ├── Query State, Query Type
│   └── Variables, NonDefaultSessionVariables
├── Planner (规划阶段)
│   ├── Parser, Analyzer, Transformer, Optimizer
│   └── HMS 调用 (getTable, getPartitionsByNames, etc.)
└── Execution (执行阶段)
    └── Fragment → Pipeline → Operator
        ├── CommonMetrics (所有算子通用)
        └── UniqueMetrics (算子特有)
```

## 2. Operator 通用指标 (CommonMetrics)

来源：`be/src/exec/pipeline/operator.cpp`

| 指标名 | 类型 | 含义 |
|-------|------|------|
| OperatorTotalTime | TIME_NS | 算子总耗时 |
| PushChunkNum | UNIT | 推送的 Chunk 数量 |
| PushRowNum | UNIT | 推送的行数 |
| PullChunkNum | UNIT | 拉取的 Chunk 数量 |
| PullRowNum | UNIT | 拉取的行数 |
| RuntimeFilterNum | UNIT | Runtime Filter 数量 |

## 3. SCAN 算子指标

### 3.1 CONNECTOR_SCAN / HDFS_SCAN / OLAP_SCAN

来源：`be/src/exec/pipeline/scan/chunk_source.cpp`, `be/src/exec/hdfs_scanner/hdfs_scanner.h`

#### IO 相关
| 指标名 | 类型 | 含义 | 诊断意义 |
|-------|------|------|---------|
| IOTaskWaitTime | TIME | IO 任务等待时间 | **关键** - IO 排队瓶颈 |
| IOTaskExecTime | TIME | IO 任务执行时间 | IO 实际读取时间 |
| AppIOTime | TIME | 应用层 IO 时间 | |
| AppIOCounter | UNIT | 应用层 IO 次数 | IO 请求数量 |
| AppIOBytesRead | BYTES | 应用层读取字节 | |
| FSIOTime | TIME | 文件系统 IO 时间 | |
| FSIOBytesRead | BYTES | 文件系统读取字节 | |

#### ORC 格式特有
来源：`be/src/exec/hdfs_scanner/hdfs_scanner_orc.cpp`

| 指标名 | 类型 | 含义 | 诊断意义 |
|-------|------|------|---------|
| TotalStripeNumber | UNIT | Stripe 总数 | **关键** - 碎片化指标 |
| TotalStripeSize | BYTES | Stripe 总大小 | |
| TotalTinyStripeSize | BYTES | 小 Stripe 总大小 | 碎片化程度 |
| StripeActiveLazyColumnIOCoalesceTogether | UNIT | 懒加载列合并读取数 | |
| StripeActiveLazyColumnIOCoalesceSeperately | UNIT | 懒加载列分离读取数 | |
| ORCSearchArgument | STRING | ORC 谓词下推表达式 | |

#### Parquet 格式特有
来源：`be/src/exec/hdfs_scanner/hdfs_scanner_parquet.cpp`

| 指标名 | 类型 | 含义 | 诊断意义 |
|-------|------|------|---------|
| TotalRowGroups | UNIT | RowGroup 总数 | **关键** - 碎片化指标 |
| FilteredRowGroups | UNIT | 被过滤的 RowGroup 数 | 过滤效果 |
| PageSkipCounter | UNIT | 跳过的 Page 数 | Page 级别过滤效果 |
| HasPageStatistics | BOOLEAN | 是否有 Page 统计 | |
| RequestBytesRead | BYTES | 请求读取字节 | |
| RequestBytesReadUncompressed | BYTES | 请求读取未压缩字节 | |
| PageReadTime | TIME | Page 读取时间 | IO 性能 |
| PageReaderCounter | UNIT | Page 读取次数 | |
| LevelDecodeTime | TIME | 层级解码时间 | |
| ValueDecodeTime | TIME | 值解码时间 | |
| FooterCacheReadCount | UNIT | Footer 缓存读取次数 | |
| FooterCacheWriteCount | UNIT | Footer 缓存写入次数 | |
| GroupChunkRead | TIME | RowGroup 读取时间 | |
| GroupDictFilter | TIME | 字典过滤时间 | |
| GroupDictDecode | TIME | 字典解码时间 | |
| GroupActiveLazyColumnIOCoalesceTogether | UNIT | 懒加载列合并读取 | |
| GroupActiveLazyColumnIOCoalesceSeperately | UNIT | 懒加载列分离读取 | |
| StatisticsTriedCounter | UNIT | 统计信息过滤尝试次数 | |
| StatisticsSuccessCounter | UNIT | 统计信息过滤成功次数 | |
| PageIndexTriedCounter | UNIT | Page 索引过滤尝试次数 | |
| PageIndexSuccessCounter | UNIT | Page 索引过滤成功次数 | |
| BloomFilterTriedCounter | UNIT | BloomFilter 过滤尝试次数 | |
| BloomFilterSuccessCounter | UNIT | BloomFilter 过滤成功次数 | |

#### DataCache 相关
| 指标名 | 类型 | 含义 | 诊断意义 |
|-------|------|------|---------|
| DataCacheReadBytes | BYTES | 缓存读取字节 | |
| DataCacheReadCounter | UNIT | 缓存读取次数 | |
| DataCacheReadMemBytes | BYTES | 内存缓存读取 | |
| DataCacheReadDiskBytes | BYTES | 磁盘缓存读取 | |
| DataCacheReadTimer | TIME | 缓存读取时间 | |
| DataCacheWriteBytes | BYTES | 缓存写入字节 | |
| DataCacheSkipReadBytes | BYTES | 跳过缓存读取 | |

#### SharedBuffered IO
| 指标名 | 类型 | 含义 |
|-------|------|------|
| SharedIOCount | UNIT | 共享 IO 次数 |
| SharedIOBytes | BYTES | 共享 IO 字节 |
| SharedAlignIOBytes | BYTES | 对齐的共享 IO 字节 |
| DirectIOCount | UNIT | 直接 IO 次数 |
| DirectIOBytes | BYTES | 直接 IO 字节 |

#### 元数据
| 指标名 | 类型 | 含义 |
|-------|------|------|
| DataSourceType | STRING | 数据源类型 (HiveDataSource 等) |
| Table | STRING | 表名 |
| ScanRanges | UNIT | 扫描范围数 |
| ScanRangesSize | BYTES | 扫描范围大小 |
| MorselsCount | UNIT | Morsel 数量 |

## 4. JOIN 算子指标

来源：`be/src/exec/pipeline/hashjoin/hash_join_*_operator.cpp`

| 指标名 | 类型 | 含义 | 诊断意义 |
|-------|------|------|---------|
| HashTableSize | UNIT | Hash 表大小 | |
| HashTableMemoryUsage | BYTES | Hash 表内存使用 | **关键** - 内存压力 |
| BuildRows | UNIT | Build 端行数 | |
| ProbeRows | UNIT | Probe 端行数 | |
| RuntimeFilterBuildTime | TIME | RF 构建时间 | |
| AvgKeysPerBucket | DOUBLE | 每桶平均键数 | Hash 碰撞指标 |

## 5. AGGREGATE 算子指标

来源：`be/src/exec/pipeline/aggregate/aggregate_*_operator.cpp`

| 指标名 | 类型 | 含义 | 诊断意义 |
|-------|------|------|---------|
| HashTableSize | UNIT | Hash 表大小 | |
| HashTableMemoryUsage | BYTES | Hash 表内存使用 | |
| InputRows | UNIT | 输入行数 | |
| OutputRows | UNIT | 输出行数 | 聚合比 |
| PassthroughRows | UNIT | 透传行数 | |

## 6. SORT 算子指标

| 指标名 | 类型 | 含义 |
|-------|------|------|
| SortKeys | UNIT | 排序键数量 |
| TotalRows | UNIT | 排序总行数 |
| TopNLimit | UNIT | TopN 限制 |

## 7. EXCHANGE 算子指标

| 指标名 | 类型 | 含义 | 诊断意义 |
|-------|------|------|---------|
| NetworkBytes | BYTES | 网络传输字节 | |
| NetworkTime | TIME | 网络传输时间 | 网络瓶颈 |
| WaitTime | TIME | 等待时间 | |

## 8. Query 级别指标

| 指标名 | 含义 | 诊断意义 |
|-------|------|---------|
| QueryCumulativeCpuTime | 累计 CPU 时间 | |
| QueryCumulativeOperatorTime | 累计算子时间 | |
| QueryAllocatedMemoryUsage | 总分配内存 | |
| QueryPeakMemoryUsage | 峰值内存 | **关键** - OOM 风险 |

## 9. Planner 阶段指标

| 指标名 | 含义 | 诊断意义 |
|-------|------|---------|
| Total | 规划总时间 | |
| Analyzer | 分析时间 | |
| Optimizer | 优化时间 | |
| RuleBaseOptimize | 规则优化时间 | |
| CostBaseOptimize | CBO 时间 | |
| HMS.getTable | 获取表元数据 | HMS 延迟 |
| HMS.getPartitionsByNames | 获取分区 | HMS 延迟 |

## 10. 指标层级关系 (Parent-Child)

```
IOTaskExecTime
├── ColumnReadTime
├── ColumnConvertTime
├── DataCache:
│   ├── DataCacheReadBytes
│   ├── DataCacheReadTimer
│   └── ...
├── ORC: (or Parquet:)
│   ├── TotalStripeNumber
│   ├── TotalStripeSize
│   ├── TotalTinyStripeSize
│   └── ...
├── InputStream:
│   ├── AppIOBytesRead
│   ├── AppIOCounter
│   ├── AppIOTime
│   ├── FSIOBytesRead
│   └── ...
└── SharedBuffered:
    ├── SharedIOBytes
    ├── SharedIOCount
    ├── DirectIOBytes
    └── ...
```

## 11. 已实现的诊断规则映射

基于源码分析，以下是指标到诊断规则的映射：

| 源码指标 | 诊断规则 | 说明 | 测试状态 |
|---------|---------|------|---------|
| TotalStripeNumber, TotalTinyStripeSize | S017 | ORC 文件碎片化 | ✅ 已测试 |
| TotalRowGroups | S017 | Parquet 文件碎片化 | ⚠️ 未测试 (无样本) |
| IOTaskWaitTime, IOTaskExecTime | S018 | IO 等待时间过长 | ✅ 已测试 |
| DataCacheReadBytes, DataCacheSkipReadBytes | S009 | 缓存命中率低 | ✅ 已测试 |
| HashTableMemoryUsage | A002, J003 | Hash 表内存过大 | ✅ 已测试 |
| ExprComputeTime | P001 | 表达式计算耗时高 | ✅ 已测试 |
| CommonSubExprComputeTime | P002 | 公共子表达式计算耗时高 | ⚠️ 新增 |
| NetworkTime, BytesSent | E001, E002 | 网络传输瓶颈 | ✅ 已测试 |
| HashTableSize, AvgKeysPerBucket | J005 | Hash 碰撞严重 | ✅ 已测试 |
| FilteredRowGroups, TotalRowGroups | S003 | 过滤效果差 | ⚠️ 未测试 |
| HMS.getTable, HMS.getPartitionsByNames | PL001 | HMS 元数据获取慢 | ⚠️ 新增 |
| Optimizer time | PL002 | 优化器耗时过长 | ⚠️ 新增 |
| Planner total time | PL003 | 规划时间占比过高 | ⚠️ 新增 |

## 12. 更新日志

| 日期 | 变更 |
|------|------|
| 2025-12-08 | 初始版本，基于 StarRocks 3.x/4.x 源码分析 |
| 2025-12-08 | 添加诊断规则映射表 |
