# Query Profile 诊断系统深度审查与改进设计

> **版本**: v2.0  
> **日期**: 2024-12-07  
> **作者**: StarRocks 高级架构师审查  
> **状态**: 审查完成，待实施改进  
> **更新**: v2.0 - 深度反思规则抑制、阈值合理性、外表类型、历史对比持久化

---

## 一、执行摘要

### 1.1 当前评分：72/100

| 维度 | 满分 | 得分 | 说明 |
|------|------|------|------|
| 规则覆盖度 | 25 | 20 | 覆盖了主要算子，但缺少关键场景 |
| 阈值合理性 | 20 | 12 | 缺乏动态阈值，硬编码过多 |
| 智能化程度 | 20 | 10 | 缺乏上下文感知，规则间缺乏关联 |
| 建议可操作性 | 15 | 12 | 建议较通用，缺乏针对性 |
| 工程实现 | 20 | 18 | 代码结构清晰，但缺少关键保护 |

### 1.2 核心问题总结

1. **P0 严重问题**：缺乏全局执行时间门槛，毫秒级查询也会产生诊断
2. **P0 严重问题**：规则缺乏绝对时间门槛，只看占比不看绝对值
3. **P1 重要问题**：缺乏查询类型感知（SELECT/INSERT/EXPORT）
4. **P1 重要问题**：规则间关系设计不当（简单抑制会丢失信息）
5. **P2 改进项**：阈值硬编码，缺少动态调整
6. **P2 改进项**：外表类型覆盖不全，缺少 HDFS_SCAN 等

---

## 二、P0 严重问题详细分析

### 2.1 问题一：缺乏全局执行时间门槛

**现状分析**：

当前 `RuleEngine::analyze_with_cluster_variables` 方法没有检查查询总执行时间。
对于毫秒级查询（如 profile2 的 11ms），仍然会触发 G001/G001b 等规则。

**问题示例**：
```
Profile2: 总执行时间 11ms
- SCHEMA_SCAN: 50.75% → 触发 G001b（次耗时节点）
- 实际只有 5.5ms，根本不需要优化
```

**修复方案**：

```rust
// backend/src/services/profile_analyzer/analyzer/rule_engine.rs

/// 全局执行时间门槛（毫秒）
const MIN_DIAGNOSIS_TIME_MS: f64 = 1000.0; // 1秒

pub fn analyze_with_cluster_variables(...) -> Vec<Diagnostic> {
    let total_time_ms = profile.summary.total_time_ms
        .or_else(|| parse_duration_ms(&profile.summary.total_time))
        .unwrap_or(0.0);
    
    if total_time_ms < MIN_DIAGNOSIS_TIME_MS {
        return vec![]; // 快速查询不需要诊断
    }
    // ... 原有逻辑
}
```

### 2.2 问题二：规则缺乏绝对时间门槛

**修复方案**：

```rust
// backend/src/services/profile_analyzer/analyzer/rules/common.rs

const MIN_OPERATOR_TIME_MS: f64 = 500.0; // 500ms

impl DiagnosticRule for G001MostConsuming {
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let percentage = context.get_time_percentage()?;
        let operator_time_ms = context.get_operator_time_ms()?;
        
        // 同时检查占比和绝对时间
        if percentage > 30.0 && operator_time_ms > MIN_OPERATOR_TIME_MS {
            Some(Diagnostic { ... })
        } else {
            None
        }
    }
}
```

---

## 三、规则关系设计（v2.0 深度反思）

### 3.1 原设计问题

原设计使用简单的"规则抑制"：
```rust
// 错误设计：S001 触发后抑制 S002
("S001", "S002", Suppresses)
```

**问题**：数据倾斜（S001）和 IO 倾斜（S002）可能是**独立问题**：
- 数据倾斜：数据分布不均（分桶键问题）
- IO 倾斜：某些节点磁盘慢（硬件问题）
- 两者可能同时存在，简单抑制会丢失重要信息

### 3.2 改进设计：规则关系类型

```rust
/// 规则关系类型
pub enum RuleRelation {
    /// 互斥：同一指标不同阈值，只保留更严重的
    /// 例：G001(>30%) 和 G001b(>15%) 互斥
    MutuallyExclusive,
    
    /// 因果：A 是 B 的根因，合并展示并标注
    /// 例：S001 数据倾斜 → G003 执行时间倾斜
    Causal { root_cause: &'static str },
    
    /// 独立：可同时存在，按优先级排序
    /// 例：S001 数据倾斜 和 S002 IO倾斜
    Independent,
}

/// 规则关系配置
pub const RULE_RELATIONS: &[(&str, &str, RuleRelation)] = &[
    // 互斥关系
    ("G001", "G001b", RuleRelation::MutuallyExclusive),
    
    // 因果关系：数据倾斜导致执行时间倾斜
    ("S001", "G003", RuleRelation::Causal { root_cause: "S001" }),
    
    // 独立关系：数据倾斜和 IO 倾斜是不同问题
    ("S001", "S002", RuleRelation::Independent),
    
    // 因果关系：Join 结果膨胀导致内存过高
    ("J001", "G002", RuleRelation::Causal { root_cause: "J001" }),
];
```

### 3.3 处理逻辑

```rust
impl RuleEngine {
    fn process_relations(&self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let mut result = Vec::new();
        let mut processed = HashSet::new();
        
        for diag in &diagnostics {
            if processed.contains(&diag.rule_id) {
                continue;
            }
            
            // 查找相关规则
            for (rule_a, rule_b, relation) in RULE_RELATIONS {
                match relation {
                    RuleRelation::MutuallyExclusive => {
                        // 只保留更严重的
                        processed.insert(rule_b);
                    }
                    RuleRelation::Causal { root_cause } => {
                        // 合并展示，标注根因
                        if &diag.rule_id == root_cause {
                            // 在建议中标注这是根因
                            diag.message = format!("🔍 根因: {}", diag.message);
                        }
                    }
                    RuleRelation::Independent => {
                        // 都保留，按优先级排序
                    }
                }
            }
            result.push(diag.clone());
        }
        result
    }
}
```

---

## 四、阈值合理性深度反思（v2.0 更新）

### 4.1 当前阈值问题汇总

| 阈值 | 当前值 | 问题 | 建议值 |
|------|--------|------|--------|
| 全局执行时间门槛 | **无** | 严重缺失 | 1s（OLAP）/ 动态（ETL） |
| G001 时间占比 | 30% | ✅ 合理 | 保持（对齐 StarRocks） |
| G002 内存 | 1GB | **太绝对** | BE 内存的 10% |
| S001 数据倾斜 | max/avg > 2 | **可能太严格** | 2.5-3.0（考虑并行度） |
| S009 缓存命中 | < 30% | **太严格** | < 50% |
| Q001 执行时间 | 60s | **太宽松** | OLAP 10s / ETL 5min |
| 小文件平均大小 | 10MB | **太严格** | 64MB（HDFS）/ 128MB（S3） |

### 4.2 动态阈值设计


```rust
/// 动态阈值计算器
pub struct DynamicThresholds {
    cluster_info: ClusterInfo,
    query_type: QueryType,
}

impl DynamicThresholds {
    /// 内存阈值：相对于 BE 内存配置
    pub fn get_memory_threshold(&self) -> u64 {
        let be_memory = self.cluster_info.be_memory_limit
            .unwrap_or(64 * 1024 * 1024 * 1024); // 默认 64GB
        (be_memory as f64 * 0.1) as u64 // 单算子不超过 BE 内存的 10%
    }
    
    /// 执行时间阈值：根据查询类型
    pub fn get_time_threshold(&self) -> f64 {
        match self.query_type {
            QueryType::Select => 10_000.0,     // OLAP: 10s
            QueryType::Insert => 300_000.0,    // ETL: 5min
            QueryType::Export => 600_000.0,    // Export: 10min
            QueryType::Analyze => 600_000.0,   // Analyze: 10min
            QueryType::Load => 1800_000.0,     // Load: 30min
            _ => 60_000.0,                     // 默认: 1min
        }
    }
    
    /// 数据倾斜阈值：根据并行度动态调整
    pub fn get_skew_threshold(&self) -> f64 {
        let parallelism = self.cluster_info.backend_num;
        match parallelism {
            p if p > 32 => 3.5,  // 大集群允许更大倾斜
            p if p > 16 => 3.0,
            p if p > 8 => 2.5,
            _ => 2.0,           // 小集群更严格
        }
    }
    
    /// 小文件阈值：根据存储类型
    pub fn get_small_file_threshold(&self, storage_type: &str) -> u64 {
        match storage_type {
            "S3" | "OSS" | "COS" | "GCS" => 128 * 1024 * 1024,  // 对象存储: 128MB
            "HDFS" => 64 * 1024 * 1024,                         // HDFS: 64MB (块大小)
            "LOCAL" => 32 * 1024 * 1024,                        // 本地: 32MB
            _ => 64 * 1024 * 1024,                              // 默认: 64MB
        }
    }
    
    /// 缓存命中率阈值：根据存储类型
    pub fn get_cache_hit_threshold(&self, is_disaggregated: bool) -> f64 {
        if is_disaggregated {
            0.5  // 存算分离：50% 命中率是警告线
        } else {
            0.3  // 共享存储：30% 命中率是警告线
        }
    }
}
```

### 4.3 阈值配置文件

```yaml
# config/diagnostic_thresholds.yaml
global:
  # 全局执行时间门槛（毫秒）
  min_diagnosis_time_ms: 1000
  # 算子绝对时间门槛（毫秒）
  min_operator_time_ms: 500

time_percentage:
  most_consuming: 30.0      # 对齐 StarRocks isMostConsuming
  second_most_consuming: 15.0  # 对齐 StarRocks isSecondMostConsuming

data_skew:
  # 根据并行度动态调整
  base_ratio: 2.0
  parallelism_factor: 0.05  # 每增加 10 并行度，阈值 +0.5

memory:
  # 相对阈值（BE 内存百分比）
  operator_peak_percent: 10
  hash_table_percent: 5
  # 绝对阈值（兜底）
  operator_peak_max: 10737418240  # 10GB
  hash_table_max: 5368709120      # 5GB

small_files:
  # 按存储类型配置
  s3:
    min_file_count: 500
    min_avg_size: 134217728  # 128MB
  hdfs:
    min_file_count: 500
    min_avg_size: 67108864   # 64MB
  local:
    min_file_count: 200
    min_avg_size: 33554432   # 32MB

cache:
  # 存算分离场景
  disaggregated_hit_rate: 0.5
  # 共享存储场景
  shared_storage_hit_rate: 0.3

cardinality:
  error_ratio: 10.0  # 实际/估算 > 10 倍
```

---

## 五、外表类型完整覆盖（v2.0 更新）

### 5.1 当前实现缺失

```rust
// 当前实现缺少多种外表类型
fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
    let name = node.operator_name.to_uppercase();
    name.contains("CONNECTOR_SCAN") || 
    name.contains("HIVE_SCAN") || 
    name.contains("ICEBERG_SCAN") ||
    name.contains("HUDI_SCAN") ||
    name.contains("DELTALAKE_SCAN")
    // 缺少: HDFS_SCAN, FILE_SCAN, PAIMON_SCAN, JDBC_SCAN 等
}
```

### 5.2 完整的外表 Scan 类型

```rust
/// 外表 Scan 类型枚举
pub enum ExternalScanType {
    // 数据湖格式
    Hive,
    Iceberg,
    Hudi,
    DeltaLake,
    Paimon,
    
    // 文件系统
    Hdfs,
    File,
    S3,
    
    // 外部数据库
    Jdbc,
    Mysql,
    Elasticsearch,
    
    // 通用连接器
    Connector,
    
    // 未知
    Unknown,
}

impl ExternalScanType {
    pub fn from_operator_name(name: &str) -> Option<Self> {
        let upper = name.to_uppercase();
        
        if upper.contains("HIVE_SCAN") { return Some(Self::Hive); }
        if upper.contains("ICEBERG_SCAN") { return Some(Self::Iceberg); }
        if upper.contains("HUDI_SCAN") { return Some(Self::Hudi); }
        if upper.contains("DELTALAKE_SCAN") { return Some(Self::DeltaLake); }
        if upper.contains("PAIMON_SCAN") { return Some(Self::Paimon); }
        if upper.contains("HDFS_SCAN") { return Some(Self::Hdfs); }
        if upper.contains("FILE_SCAN") { return Some(Self::File); }
        if upper.contains("S3_SCAN") { return Some(Self::S3); }
        if upper.contains("JDBC_SCAN") { return Some(Self::Jdbc); }
        if upper.contains("MYSQL_SCAN") { return Some(Self::Mysql); }
        if upper.contains("ES_SCAN") { return Some(Self::Elasticsearch); }
        if upper.contains("CONNECTOR_SCAN") { return Some(Self::Connector); }
        
        None
    }
    
    /// 是否适用小文件检测
    pub fn supports_small_file_detection(&self) -> bool {
        matches!(self, 
            Self::Hive | Self::Iceberg | Self::Hudi | 
            Self::DeltaLake | Self::Paimon | Self::Hdfs | 
            Self::File | Self::S3 | Self::Connector
        )
    }
    
    /// 获取存储类型（用于阈值计算）
    pub fn storage_type(&self) -> &'static str {
        match self {
            Self::S3 => "S3",
            Self::Hdfs | Self::Hive => "HDFS",
            Self::Iceberg | Self::Hudi | Self::DeltaLake | Self::Paimon => "HDFS", // 通常基于 HDFS
            Self::File => "LOCAL",
            _ => "UNKNOWN",
        }
    }
    
    /// 获取小文件检测的指标名
    pub fn file_count_metric(&self) -> &'static str {
        match self {
            Self::Hdfs => "BlocksRead",
            _ => "ScanRanges",
        }
    }
}
```

### 5.3 更新后的小文件检测规则

```rust
/// S016: 外表小文件检测（v2.0 更新）
pub struct S016ExternalSmallFiles;

impl DiagnosticRule for S016ExternalSmallFiles {
    fn id(&self) -> &str { "S016" }
    fn name(&self) -> &str { "外表小文件过多" }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        ExternalScanType::from_operator_name(&node.operator_name)
            .map(|t| t.supports_small_file_detection())
            .unwrap_or(false)
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let scan_type = ExternalScanType::from_operator_name(
            &context.node.operator_name
        )?;
        
        // 获取文件数量指标
        let metric_name = scan_type.file_count_metric();
        let file_count = context.get_metric(metric_name)
            .or_else(|| context.get_metric("MorselsCount"))?;
        
        let bytes_read = context.get_metric("BytesRead").unwrap_or(0.0);
        
        if file_count < 100.0 { return None; }
        
        let avg_file_size = if file_count > 0.0 { 
            bytes_read / file_count 
        } else { 
            0.0 
        };
        
        // 根据存储类型获取阈值
        let storage_type = scan_type.storage_type();
        let threshold = context.get_small_file_threshold(storage_type);
        
        if file_count > 500.0 && avg_file_size < threshold as f64 {
            let table_name = context.node.unique_metrics
                .get("Table")
                .map(|s| s.as_str())
                .unwrap_or("external_table");
            
            // 根据外表类型生成针对性建议
            let suggestions = generate_small_file_suggestions(&scan_type, table_name);
            
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "扫描了 {:.0} 个文件，平均大小仅 {}（建议 > {}）",
                    file_count, 
                    format_bytes(avg_file_size as u64),
                    format_bytes(threshold)
                ),
                reason: format!(
                    "外表 {} 存在大量小文件，导致元数据开销大、IO 效率低。",
                    table_name
                ),
                suggestions,
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

fn generate_small_file_suggestions(scan_type: &ExternalScanType, table: &str) -> Vec<String> {
    match scan_type {
        ExternalScanType::Hive => vec![
            format!("合并小文件: INSERT OVERWRITE {} SELECT * FROM {}", table, table),
            "调整 Hive 表的 mapreduce.input.fileinputformat.split.minsize".to_string(),
        ],
        ExternalScanType::Iceberg => vec![
            format!("执行 Compaction: CALL rewrite_data_files(table => '{}')", table),
            "调整 write.target-file-size-bytes 参数".to_string(),
        ],
        ExternalScanType::Hudi => vec![
            "执行 Hudi Compaction 合并小文件".to_string(),
            "调整 hoodie.parquet.small.file.limit 参数".to_string(),
        ],
        ExternalScanType::DeltaLake => vec![
            format!("执行 OPTIMIZE {} ZORDER BY ...", table),
            "启用 Auto Compaction".to_string(),
        ],
        ExternalScanType::Hdfs => vec![
            "使用 Hadoop Archive (HAR) 合并小文件".to_string(),
            "调整上游 ETL 输出文件大小（建议 128MB-256MB）".to_string(),
        ],
        _ => vec![
            "合并小文件以提升查询性能".to_string(),
            "考虑将热点数据导入 StarRocks 内表".to_string(),
        ],
    }
}
```

---

## 六、查询指纹与历史对比（v2.0 深度设计）

### 6.1 是否需要持久化？

| 场景 | 是否需要持久化 | 存储方案 |
|------|--------------|---------|
| 单次诊断 | ❌ 不需要 | - |
| 会话内对比 | ❌ 不需要 | 内存缓存 |
| 跨会话对比 | ✅ 需要 | 本地 SQLite |
| 多用户共享 | ✅ 需要 | 远程存储 |
| 生产监控 | ✅ 需要 | 远程存储 + 告警 |

**建议分阶段实施**：
- **MVP**：只用内存缓存，不持久化
- **V1**：本地 SQLite 持久化
- **V2**：可选远程存储（如 StarRocks 自身）

### 6.2 存储架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    查询历史存储架构                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │  内存缓存    │    │  本地存储    │    │  远程存储    │     │
│  │  (LRU)      │───▶│  (SQLite)   │───▶│  (可选)     │     │
│  │  10K 条目   │    │  30 天保留   │    │  StarRocks  │     │
│  └─────────────┘    └─────────────┘    └─────────────┘     │
│         │                  │                  │             │
│         ▼                  ▼                  ▼             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                QueryHistoryService                   │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 数据结构设计

```rust
/// 查询指纹（用于识别相似查询）
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct QueryFingerprint {
    /// SQL 模板（参数化后）
    /// "SELECT * FROM t WHERE id = 123" → "SELECT * FROM t WHERE id = ?"
    pub sql_template: String,
    /// 涉及的表（排序后）
    pub tables: Vec<String>,
    /// 查询类型
    pub query_type: QueryType,
}

impl QueryFingerprint {
    pub fn from_profile(profile: &Profile) -> Self {
        let sql = profile.summary.sql.as_deref().unwrap_or("");
        Self {
            sql_template: Self::normalize_sql(sql),
            tables: Self::extract_tables(sql),
            query_type: QueryType::from_profile(&profile.summary),
        }
    }
    
    /// SQL 参数化：将具体值替换为占位符
    fn normalize_sql(sql: &str) -> String {
        // 简化实现，实际需要更复杂的 SQL 解析
        let mut result = sql.to_string();
        // 替换数字
        result = regex::Regex::new(r"\b\d+\b").unwrap()
            .replace_all(&result, "?").to_string();
        // 替换字符串
        result = regex::Regex::new(r"'[^']*'").unwrap()
            .replace_all(&result, "?").to_string();
        result
    }
}

/// 执行基线（聚合统计）
pub struct ExecutionBaseline {
    pub fingerprint_hash: u64,
    pub sample_count: u32,
    pub time_stats: TimeStats,
    pub resource_stats: ResourceStats,
    pub last_updated: DateTime<Utc>,
}

pub struct TimeStats {
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p99_ms: f64,
    pub avg_ms: f64,
}

pub struct ResourceStats {
    pub avg_memory_bytes: u64,
    pub avg_scan_bytes: u64,
    pub avg_shuffle_bytes: u64,
}
```

### 6.4 存储策略配置

```rust
pub struct HistoryConfig {
    /// 内存缓存大小（指纹数量）
    pub memory_cache_size: usize,      // 默认 10000
    
    /// 本地存储保留天数
    pub local_retention_days: u32,     // 默认 30
    
    /// 是否启用远程存储
    pub enable_remote_storage: bool,   // 默认 false
    
    /// 采样率（避免存储过多）
    pub sampling_rate: f64,            // 默认 0.1 (10%)
    
    /// 最小执行时间（太快的不记录）
    pub min_record_time_ms: f64,       // 默认 100ms
    
    /// 最小样本数（样本太少不判断回归）
    pub min_samples_for_regression: u32, // 默认 10
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            memory_cache_size: 10000,
            local_retention_days: 30,
            enable_remote_storage: false,
            sampling_rate: 0.1,
            min_record_time_ms: 100.0,
            min_samples_for_regression: 10,
        }
    }
}
```

### 6.5 回归检测逻辑

```rust
impl QueryHistoryService {
    /// 检测性能回归
    pub fn detect_regression(
        &self,
        fingerprint: &QueryFingerprint,
        current_time_ms: f64,
    ) -> Option<RegressionDiagnostic> {
        let baseline = self.get_baseline(fingerprint)?;
        
        // 样本太少不判断
        if baseline.sample_count < self.config.min_samples_for_regression {
            return None;
        }
        
        // 计算回归比率（与 P90 对比）
        let ratio = current_time_ms / baseline.time_stats.p90_ms;
        
        if ratio > 2.0 {
            Some(RegressionDiagnostic {
                rule_id: "REG001".to_string(),
                rule_name: "性能回归".to_string(),
                severity: if ratio > 5.0 { 
                    RuleSeverity::Error 
                } else { 
                    RuleSeverity::Warning 
                },
                message: format!(
                    "查询执行时间 {:.1}ms，是历史 P90（{:.1}ms）的 {:.1} 倍",
                    current_time_ms, baseline.time_stats.p90_ms, ratio
                ),
                baseline_p90_ms: baseline.time_stats.p90_ms,
                current_ms: current_time_ms,
                regression_ratio: ratio,
                sample_count: baseline.sample_count,
            })
        } else {
            None
        }
    }
    
    /// 记录执行（采样）
    pub fn record_execution(&self, fingerprint: &QueryFingerprint, metrics: &ExecutionMetrics) {
        // 采样控制
        if rand::random::<f64>() > self.config.sampling_rate {
            return;
        }
        
        // 太快的不记录
        if metrics.total_time_ms < self.config.min_record_time_ms {
            return;
        }
        
        // 更新基线
        self.update_baseline(fingerprint, metrics);
    }
}
```

---

## 七、查询类型感知


### 7.1 查询类型定义

```rust
/// 查询类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryType {
    Select,     // 普通查询
    Insert,     // INSERT INTO SELECT
    Export,     // EXPORT 导出
    Analyze,    // ANALYZE TABLE
    Ctas,       // CREATE TABLE AS SELECT
    Load,       // Broker Load / Routine Load
    Unknown,
}

impl QueryType {
    pub fn from_profile(summary: &ProfileSummary) -> Self {
        let sql = summary.sql.as_deref().unwrap_or("").to_uppercase();
        let sql = sql.trim();
        
        if sql.starts_with("INSERT") {
            QueryType::Insert
        } else if sql.starts_with("EXPORT") {
            QueryType::Export
        } else if sql.starts_with("ANALYZE") {
            QueryType::Analyze
        } else if sql.starts_with("CREATE TABLE") && sql.contains("AS SELECT") {
            QueryType::Ctas
        } else if sql.starts_with("LOAD") || sql.contains("BROKER LOAD") {
            QueryType::Load
        } else if sql.starts_with("SELECT") {
            QueryType::Select
        } else {
            QueryType::Unknown
        }
    }
}
```

### 7.2 查询类型特定配置

```rust
impl QueryType {
    /// 获取执行时间阈值（毫秒）
    pub fn get_time_threshold(&self) -> f64 {
        match self {
            QueryType::Select => 10_000.0,     // OLAP: 10s
            QueryType::Insert => 300_000.0,    // ETL: 5min
            QueryType::Export => 600_000.0,    // Export: 10min
            QueryType::Analyze => 600_000.0,   // Analyze: 10min
            QueryType::Ctas => 300_000.0,      // CTAS: 5min
            QueryType::Load => 1800_000.0,     // Load: 30min
            QueryType::Unknown => 60_000.0,    // 默认: 1min
        }
    }
    
    /// 获取适用的规则集
    pub fn applicable_rules(&self) -> Vec<&'static str> {
        match self {
            QueryType::Select => vec![
                "G001", "G001b", "G002", "G003",  // 通用规则
                "S001", "S003", "S007", "S009",   // Scan 规则
                "J001", "J002", "J004",           // Join 规则
                "Q001", "Q002", "Q005",           // Query 规则
            ],
            QueryType::Insert | QueryType::Ctas => vec![
                "G001", "G002", "G003",           // 通用规则（不含 G001b）
                "S001", "S007",                   // Scan 规则（不含过滤效果）
                "I001", "I002", "I003",           // Sink 规则
                // 不包含 Q001（执行时间阈值不同）
            ],
            QueryType::Export => vec![
                "G002", "G003",                   // 内存和倾斜
                // IO 占比高是正常的，不检测
            ],
            QueryType::Analyze => vec![
                "G002",                           // 只检测内存
                // 扫描量大是正常的
            ],
            QueryType::Load => vec![
                "I001", "I002", "I003",           // 只检测导入相关
            ],
            QueryType::Unknown => vec![
                "G001", "G001b", "G002", "G003",  // 所有通用规则
            ],
        }
    }
    
    /// 是否应该跳过某个规则
    pub fn should_skip_rule(&self, rule_id: &str) -> bool {
        !self.applicable_rules().contains(&rule_id)
    }
}
```

---

## 八、新增规则设计

### 8.1 小文件检测规则（S015/S016）

见第五节的完整实现。

### 8.2 统计信息规则（STAT001/STAT002）

```rust
/// STAT001: 基数估算偏差大
pub struct STAT001CardinalityError;

impl DiagnosticRule for STAT001CardinalityError {
    fn id(&self) -> &str { "STAT001" }
    fn name(&self) -> &str { "基数估算偏差大" }

    fn applicable_to(&self, _node: &ExecutionTreeNode) -> bool {
        true
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let estimated = context.get_metric("EstimatedRows")
            .or_else(|| context.get_metric("Cardinality"))?;
        let actual = context.node.rows.unwrap_or(0) as f64;
        
        if estimated <= 0.0 || actual <= 0.0 { return None; }
        
        let ratio = (actual / estimated).max(estimated / actual);
        
        if ratio > 10.0 {
            let table_name = context.node.unique_metrics
                .get("Table")
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "基数估算偏差 {:.1} 倍（实际 {:.0} 行，估算 {:.0} 行）",
                    ratio, actual, estimated
                ),
                reason: "优化器基数估算与实际执行结果偏差过大，可能导致执行计划不优。".to_string(),
                suggestions: vec![
                    format!("执行 ANALYZE TABLE {}; 更新统计信息", table_name),
                    "检查统计信息收集时间: SHOW STATS META".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}
```

### 8.3 分区裁剪规则（PART001）

```rust
/// PART001: 分区裁剪未生效
pub struct PART001PartitionPruning;

impl DiagnosticRule for PART001PartitionPruning {
    fn id(&self) -> &str { "PART001" }
    fn name(&self) -> &str { "分区裁剪未生效" }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let scanned = context.get_metric("PartitionsScanned")?;
        let total = context.get_metric("TotalPartitions")?;
        
        if total < 10.0 { return None; } // 分区太少不检测
        
        let ratio = scanned / total;
        
        if ratio > 0.5 {
            let table_name = context.node.unique_metrics
                .get("Table")
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "扫描了 {:.0}/{:.0} 个分区 ({:.1}%)",
                    scanned, total, ratio * 100.0
                ),
                reason: "分区裁剪未能有效减少扫描范围，可能是 WHERE 条件未包含分区键。".to_string(),
                suggestions: vec![
                    "检查 WHERE 条件是否包含分区键".to_string(),
                    "检查分区键类型是否匹配（避免隐式转换）".to_string(),
                    format!("查看分区信息: SHOW PARTITIONS FROM {}", table_name),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}
```

---

## 九、单元测试改进

### 9.1 关键测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// P0: 快速查询不应产生诊断
    #[test]
    fn test_fast_query_no_diagnostics() {
        let profile = create_test_profile_with_time("11ms");
        let engine = RuleEngine::new();
        let diagnostics = engine.analyze(&profile);
        
        assert!(diagnostics.is_empty(),
            "快速查询（11ms）不应产生诊断，但得到了 {} 条", diagnostics.len());
    }

    /// P0: 算子绝对时间门槛
    #[test]
    fn test_operator_absolute_time_threshold() {
        let profile = create_test_profile_with_operator(100.0, 50.0); // 100ms 查询，算子占 50%
        let engine = RuleEngine::new();
        let diagnostics = engine.analyze(&profile);
        
        let g001 = diagnostics.iter().find(|d| d.rule_id == "G001");
        assert!(g001.is_none(), "50ms 的算子不应触发 G001");
    }

    /// 规则关系：互斥
    #[test]
    fn test_rule_mutual_exclusion() {
        let profile = create_test_profile_with_high_percentage(35.0);
        let engine = RuleEngine::new();
        let diagnostics = engine.analyze(&profile);
        
        let g001 = diagnostics.iter().filter(|d| d.rule_id == "G001").count();
        let g001b = diagnostics.iter().filter(|d| d.rule_id == "G001b").count();
        
        assert!(g001 > 0, "G001 应该触发");
        assert_eq!(g001b, 0, "G001b 应该被 G001 互斥");
    }

    /// 规则关系：独立（S001 和 S002 可同时存在）
    #[test]
    fn test_rule_independence() {
        let profile = create_test_profile_with_both_skews();
        let engine = RuleEngine::new();
        let diagnostics = engine.analyze(&profile);
        
        let s001 = diagnostics.iter().any(|d| d.rule_id == "S001");
        let s002 = diagnostics.iter().any(|d| d.rule_id == "S002");
        
        // 两者可以同时存在
        assert!(s001 || s002, "至少应触发一个倾斜规则");
    }

    /// 查询类型感知
    #[test]
    fn test_query_type_awareness() {
        let profile = create_test_profile_with_sql("INSERT INTO t1 SELECT * FROM t2");
        let query_type = QueryType::from_profile(&profile.summary);
        
        assert_eq!(query_type, QueryType::Insert);
        assert_eq!(query_type.get_time_threshold(), 300_000.0);
        assert!(query_type.should_skip_rule("Q001")); // INSERT 不检测 Q001
    }

    /// 外表小文件检测
    #[test]
    fn test_external_small_files() {
        let node = create_test_node("HDFS_SCAN", vec![
            ("BlocksRead", "2000"),
            ("BytesRead", "1073741824"), // 1GB / 2000 = 512KB avg
        ]);
        
        let rule = S016ExternalSmallFiles;
        let context = create_test_context(&node);
        let result = rule.evaluate(&context);
        
        assert!(result.is_some(), "应检测到 HDFS 小文件问题");
    }

    /// 动态阈值：内存
    #[test]
    fn test_dynamic_memory_threshold() {
        let cluster_info = ClusterInfo {
            backend_num: 8,
            be_memory_limit: Some(128 * 1024 * 1024 * 1024), // 128GB
            ..Default::default()
        };
        
        let thresholds = DynamicThresholds::new(cluster_info, QueryType::Select);
        let memory_threshold = thresholds.get_memory_threshold();
        
        // 128GB * 10% = 12.8GB
        assert_eq!(memory_threshold, 12 * 1024 * 1024 * 1024 + 800 * 1024 * 1024);
    }

    /// 动态阈值：数据倾斜
    #[test]
    fn test_dynamic_skew_threshold() {
        let small_cluster = ClusterInfo { backend_num: 4, ..Default::default() };
        let large_cluster = ClusterInfo { backend_num: 64, ..Default::default() };
        
        let small_threshold = DynamicThresholds::new(small_cluster, QueryType::Select)
            .get_skew_threshold();
        let large_threshold = DynamicThresholds::new(large_cluster, QueryType::Select)
            .get_skew_threshold();
        
        assert!(large_threshold > small_threshold, 
            "大集群应允许更大的倾斜阈值");
    }
}
```

---

## 十、实施计划（更新）

### 10.1 优先级排序

| 优先级 | 改进项 | 预估工作量 | 收益 |
|--------|--------|-----------|------|
| **P0** | 全局执行时间门槛 | 0.5天 | 避免误报 |
| **P0** | 规则绝对时间门槛 | 1天 | 避免误报 |
| **P0** | 新增单元测试 | 1天 | 质量保证 |
| **P1** | 查询类型感知 | 1天 | 减少噪音 |
| **P1** | 规则关系重构（互斥/因果/独立） | 2天 | 提升准确性 |
| **P1** | 外表类型完善（HDFS_SCAN 等） | 0.5天 | 覆盖完整 |
| **P1** | 小文件检测规则 | 1天 | 覆盖关键场景 |
| **P2** | 动态阈值实现 | 2天 | 更智能 |
| **P2** | 统计信息规则 | 1天 | 覆盖关键场景 |
| **P3** | 历史对比（内存缓存） | 2天 | 基础功能 |
| **P3** | 历史对比（SQLite 持久化） | 3天 | 完整功能 |

### 10.2 分阶段实施

**第一阶段（P0，2.5 天）**：
1. 全局执行时间门槛
2. 算子绝对时间门槛
3. 关键单元测试

**第二阶段（P1，5.5 天）**：
1. 查询类型感知
2. 规则关系重构
3. 外表类型完善
4. 小文件检测规则

**第三阶段（P2，3 天）**：
1. 动态阈值实现
2. 统计信息规则

**第四阶段（P3，5 天）**：
1. 历史对比（内存缓存）
2. 历史对比（SQLite 持久化）

---

## 十一、完整规则清单

| 规则ID | 名称 | 类型 | 状态 | 备注 |
|--------|------|------|------|------|
| G001 | 算子时间占比过高 | 通用 | ✅ 需修改 | 添加绝对时间门槛 |
| G001b | 算子时间占比较高 | 通用 | ✅ 需修改 | 添加绝对时间门槛 |
| G002 | 算子内存使用过高 | 通用 | ✅ 需修改 | 改为动态阈值 |
| G003 | 算子执行时间倾斜 | 通用 | ✅ 需修改 | 动态倾斜阈值 |
| S001-S014 | Scan 规则 | Scan | ✅ 已实现 | - |
| **S015** | 内表 Rowset 碎片化 | Scan | 🆕 待实现 | - |
| **S016** | 外表小文件过多 | Scan | 🆕 待实现 | 支持 HDFS_SCAN |
| J001-J010 | Join 规则 | Join | ✅ 已实现 | - |
| A001-A005 | Aggregate 规则 | Aggregate | ✅ 已实现 | - |
| T001-T005 | Sort 规则 | Sort | ✅ 已实现 | - |
| W001 | 窗口分区过宽 | Window | ✅ 已实现 | - |
| E001-E003 | Exchange 规则 | Exchange | ✅ 已实现 | - |
| Q001-Q009 | Query 规则 | Query | ✅ 需修改 | 查询类型感知 |
| I001-I003 | Sink 规则 | Sink | ✅ 已实现 | - |
| P001 | Project 表达式计算慢 | Project | ✅ 已实现 | - |
| L001 | LocalExchange 内存过高 | LocalExchange | ✅ 已实现 | - |
| **STAT001** | 基数估算偏差大 | 通用 | 🆕 待实现 | - |
| **STAT002** | 统计信息缺失 | Scan | 🆕 待实现 | - |
| **PART001** | 分区裁剪未生效 | Scan | 🆕 待实现 | - |
| **COL001** | 可使用 Colocate Join | Join | 🆕 待实现 | - |
| **REG001** | 性能回归 | Query | 🆕 待实现 | 需要历史对比 |

---

## 十二、总结

### 12.1 关键改进点

1. **规则抑制 → 规则关系**：从简单抑制改为互斥/因果/独立三种关系，避免丢失信息
2. **固定阈值 → 动态阈值**：内存、倾斜、小文件阈值根据集群配置和存储类型动态调整
3. **外表类型完善**：补充 HDFS_SCAN、PAIMON_SCAN 等，并针对不同类型生成针对性建议
4. **历史对比分层**：MVP 用内存缓存，后续按需添加 SQLite 持久化

### 12.2 预期效果

通过实施以上改进，预计：
- 误报率降低 80%（全局时间门槛 + 绝对时间门槛）
- 诊断准确性提升 30%（规则关系重构 + 动态阈值）
- 覆盖场景增加 20%（新增规则 + 外表类型完善）
- 系统评分从 72 分提升至 **90+ 分**
