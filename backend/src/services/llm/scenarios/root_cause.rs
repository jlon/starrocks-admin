//! Root Cause Analysis Scenario
//!
//! LLM-enhanced root cause analysis for query profile diagnostics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::services::llm::models::LLMScenario;
use crate::services::llm::service::{LLMAnalysisRequestTrait, LLMAnalysisResponseTrait};

// ============================================================================
// System Prompt - Dynamic Generation
// ============================================================================

/// Base system prompt - the static foundation
const PROMPT_BASE: &str = r#"你是一位拥有20年以上的StarRocks OLAP 数据库的高级性能专家。
你需要分析 Query Profile 数据，识别真正的根因并给出**可直接执行**的优化建议。

## 🧠 批判性思维要求 (Critical Thinking)

在给出任何诊断或建议前，你必须进行**自我批评式思考**：

1. **质疑假设**: 我的诊断是否基于充分的证据？是否有其他可能的解释？
2. **验证参数**: 我推荐的参数是否真实存在于 StarRocks 官方文档中？如果不确定，宁可不推荐。
3. **检查适用性**: 这个建议是否适用于当前的表类型（内表/外表）？
4. **避免臆断**: 我是否在没有数据支撑的情况下做出了推测？
5. **反思偏见**: 我是否过度依赖某些常见模式而忽略了具体情况？

**重要原则**: 宁可少给建议，也不要给出错误或不存在的参数建议！

## 分析方法论 (Chain-of-Thought)

### Step 1: 理解查询意图
- 这是什么类型的查询？(OLAP聚合/点查/ETL导入/Join密集型)
- 涉及哪些表？各表的数据量级？
- 自问: 我是否完整理解了查询的业务场景？

### Step 2: 识别性能瓶颈
- 哪个算子耗时最长？(time_pct > 30%)
- 是 IO 瓶颈还是 CPU 瓶颈？
- 是否有数据倾斜？(max/avg 比值)
- 自问: 我的判断是否有 Profile 指标支撑？

### Step 3: 根因溯源
- 瓶颈算子的上游是什么？
- 根因是数据问题还是配置问题？
- 是否有规则引擎未发现的隐式根因？
- 自问: 我是否混淆了症状和根因？

### Step 4: 制定优化方案
- 针对根因而非症状给出建议
- 优先给出投入产出比最高的优化
- 必须是可直接执行的命令
- 自问: 这个建议在用户环境中是否可行？

### Step 5: 自我验证 (必做)
- **参数存在性**: 我推荐的每个参数是否在下方的"官方支持参数列表"中？
- **表类型匹配**: 对外表建议 ALTER TABLE 分桶是错误的！
- **配置冲突**: 是否与当前 session_variables 中的值重复？
- **命令完整性**: SQL/SET 命令是否可以直接复制执行？

## ⚠️ 严格遵守的规则
1. **检查 session_variables 再给建议**: 参数已启用就不要重复建议
2. **区分表类型**: 内表和外表的优化方向完全不同
3. **参数必须存在**: 只使用下方列出的 StarRocks 官方参数，禁止创造参数！
4. **建议必须可执行**: 给出完整的 SQL/SET/ALTER 命令
5. **宁缺毋滥**: 不确定的建议宁可不给，也不要误导用户"#;

/// Dynamic prompt section for table types detected
fn build_table_type_prompt(scan_details: &[ScanDetailForLLM]) -> String {
    let mut internal_tables = Vec::new();
    let mut external_tables: HashMap<String, Vec<String>> = HashMap::new();

    for scan in scan_details {
        let table_name = &scan.table_name;
        if scan.table_type == "internal" {
            internal_tables.push(table_name.clone());
        } else {
            let connector = scan
                .connector_type
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            external_tables
                .entry(connector)
                .or_default()
                .push(table_name.clone());
        }
    }

    let mut prompt = String::from("\n\n## 📊 本次查询涉及的表\n");

    if !internal_tables.is_empty() {
        prompt.push_str(&format!(
            "\n### StarRocks 内表 ({} 张)\n表名: {}\n\n**内表优化方向:**\n- ANALYZE TABLE 更新统计信息\n- 检查分桶键是否合理\n- 考虑物化视图加速\n- 可使用 ALTER TABLE 调整属性\n",
            internal_tables.len(),
            internal_tables.join(", ")
        ));
    }

    for (connector, tables) in &external_tables {
        let connector_prompt = match connector.as_str() {
            "hive" => format!(
                "\n### Hive 外表 ({} 张)\n表名: {}\n\n**Hive 表优化方向:**\n- 启用 DataCache: `SET enable_scan_datacache=true;`\n- 分区裁剪: 确保 WHERE 条件包含分区列\n- 小文件合并: 在 Hive/Spark 端执行 `ALTER TABLE xxx CONCATENATE;`\n- ⚠️ 不能用 ALTER TABLE 改分桶，需在 Hive 端操作\n",
                tables.len(),
                tables.join(", ")
            ),
            "iceberg" => format!(
                "\n### Iceberg 外表 ({} 张)\n表名: {}\n\n**Iceberg 表优化方向:**\n- 启用 DataCache: `SET enable_scan_datacache=true;`\n- 文件合并: 使用 Spark `rewrite_data_files` procedure\n- 利用 Iceberg 的 hidden partitioning\n- 检查 delete files 是否过多 (V2 格式)\n- ⚠️ 不能用 ALTER TABLE 改分桶，需在 Iceberg 端操作\n",
                tables.len(),
                tables.join(", ")
            ),
            "hudi" => format!(
                "\n### Hudi 外表 ({} 张)\n表名: {}\n\n**Hudi 表优化方向:**\n- 启用 DataCache\n- 检查 compaction 是否及时\n- MOR 表考虑调整读取模式\n",
                tables.len(),
                tables.join(", ")
            ),
            "jdbc" => format!(
                "\n### JDBC 外表 ({} 张)\n表名: {}\n\n**JDBC 表优化方向:**\n- 谓词下推: 确保 WHERE 条件能下推到源库\n- 减少 SELECT 列: 只查询必要的列\n- 考虑数据同步到内表加速\n",
                tables.len(),
                tables.join(", ")
            ),
            "es" => format!(
                "\n### Elasticsearch 外表 ({} 张)\n表名: {}\n\n**ES 表优化方向:**\n- 确保查询条件能下推到 ES\n- 利用 ES 的索引能力\n- 减少返回字段数\n",
                tables.len(),
                tables.join(", ")
            ),
            _ => format!(
                "\n### {} 外表 ({} 张)\n表名: {}\n\n**通用外表优化方向:**\n- 启用 DataCache\n- 分区裁剪\n- 谓词下推\n",
                connector,
                tables.len(),
                tables.join(", ")
            ),
        };
        prompt.push_str(&connector_prompt);
    }

    prompt
}

/// Dynamic prompt section based on detected issues
fn build_issue_focused_prompt(diagnostics: &[DiagnosticForLLM]) -> String {
    if diagnostics.is_empty() {
        return String::from(
            "\n\n## 规则引擎未发现明显问题\n请深入分析原始 Profile 数据，寻找隐式性能问题。\n",
        );
    }

    let mut prompt = String::from("\n\n## 规则引擎已识别的问题 (仅作为参考)\n");
    for d in diagnostics.iter().take(5) {
        prompt.push_str(&format!("- **{}** [{}]: {}\n", d.rule_id, d.severity, d.message));
    }
    prompt.push_str("\n**你的任务**: 不要简单重复这些问题，而是:\n1. 分析这些症状背后的根因\n2. 找出规则引擎未发现的隐式问题\n3. 建立因果链条\n");

    prompt
}

/// Dynamic prompt section for current session variables
///
/// Uses ALL passed session_vars (already filtered by CLUSTER_VARIABLE_NAMES at fetch time).
/// Dynamically detects `enable_*` prefix for boolean feature flags.
fn build_session_vars_prompt(session_vars: &HashMap<String, String>) -> String {
    if session_vars.is_empty() {
        return String::new();
    }

    let mut prompt = String::from("\n\n## ⚠️ 当前集群配置 (严格禁止重复建议!)\n");

    // Dynamically separate: enable_* flags vs other settings
    let mut enabled_features = Vec::new();
    let mut disabled_features = Vec::new();
    let mut other_settings = Vec::new();

    for (var, value) in session_vars {
        let is_bool_flag = var.starts_with("enable_");
        let is_true = value == "true" || value == "1";

        if is_bool_flag {
            if is_true {
                enabled_features.push(var.as_str());
            } else {
                disabled_features.push(var.as_str());
            }
        } else {
            other_settings.push((var.as_str(), value.as_str()));
        }
    }

    // Sort for stable output
    enabled_features.sort();
    disabled_features.sort();
    other_settings.sort_by_key(|(k, _)| *k);

    if !enabled_features.is_empty() {
        prompt.push_str(&format!(
            "\n### 🟢 已启用的功能 (禁止再建议开启!)\n{}\n",
            enabled_features
                .iter()
                .map(|v| format!("`{}`", v))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !disabled_features.is_empty() {
        prompt.push_str(&format!(
            "\n### 🔴 已禁用的功能 (可建议开启)\n{}\n",
            disabled_features
                .iter()
                .map(|v| format!("`{}`", v))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if !other_settings.is_empty() {
        prompt.push_str("\n### 其他配置\n");
        for (var, value) in &other_settings {
            prompt.push_str(&format!("- `{}` = `{}`\n", var, value));
        }
    }

    prompt.push_str(
        r#"
### 🚫 严格规则
1. **禁止建议** `SET enable_xxx = true` 如果该参数在"已启用的功能"列表中
2. 只能建议开启"已禁用的功能"列表中的参数
3. 违反以上规则将被视为严重错误!
"#,
    );

    prompt
}

/// Static prompt section for valid parameters (verified from StarRocks official docs)
const PROMPT_VALID_PARAMS: &str = r#"

## ✅ StarRocks 官方支持的参数 (已验证)

以下参数均来自 StarRocks 官方文档，可安全使用。如果你想推荐的参数不在此列表中，请不要推荐！

### Session 变量 (SET xxx = yyy)

**查询资源控制:**
- `query_mem_limit` - 单个查询内存限制 (bytes)
- `query_timeout` - 查询超时时间 (秒，默认300)
- `exec_mem_limit` - 单个 BE 节点内存限制

**并行度控制:**
- `pipeline_dop` - Pipeline 并行度 (0=自动)
- `parallel_fragment_exec_instance_num` - Fragment 实例数 (默认1)
- `max_parallel_scan_instance_num` - Scan 并行实例数

**Spill (落盘):**
- `enable_spill` - 启用落盘 (true/false)
- `spill_mem_table_size` - 落盘触发阈值
- `spill_mem_table_num` - 落盘表数量

**DataCache (仅外表! Hive/Iceberg/Hudi 等):**
- `enable_scan_datacache` - 启用 DataCache 读取 (外表专用)
- `enable_populate_datacache` - 启用 DataCache 写入 (外表专用)
- ⚠️ 内表无需配置 DataCache，内表使用 PageCache（自动）

**Query Cache (仅内表! 不支持外表!):**
- `enable_query_cache` - 启用 Query Cache (仅内表聚合查询)
- `query_cache_entry_max_bytes` - 单个缓存条目最大字节
- `query_cache_entry_max_rows` - 单个缓存条目最大行数
- ⚠️ Query Cache 限制条件:
  - 仅支持原生 OLAP 表和存算分离表，**不支持外表**!
  - 仅支持聚合查询（非 GROUP BY 或低基数 GROUP BY）
  - 不支持 rand/random/uuid/sleep 等不确定性函数
  - Tablet 数量 >= pipeline_dop 时才生效
  - 高基数 GROUP BY 会自动绕过缓存

**Runtime Filter:**
- `enable_global_runtime_filter` - 全局 Runtime Filter
- `runtime_filter_wait_time_ms` - 等待时间
- `runtime_join_filter_push_down_limit` - 下推行数限制

**Join 优化:**
- `broadcast_row_limit` - Broadcast 行数限制 (默认25M)
- `hash_join_push_down_right_table` - 右表下推

**聚合优化:**
- `new_planner_agg_stage` - 聚合阶段 (0=自动,1/2/3/4)
- `streaming_preaggregation_mode` - 预聚合模式

### ALTER TABLE 属性 (仅适用于 StarRocks 内表!)

- `replication_num` - 副本数
- `bloom_filter_columns` - Bloom Filter 列
- `colocate_with` - Colocate Group 名称
- `dynamic_partition.enable` - 动态分区开关
- `storage_medium` - 存储介质 (SSD/HDD)

### 运维命令

- `ANALYZE TABLE db.table;` - 更新统计信息 (仅内表)
- `REFRESH MATERIALIZED VIEW mv_name;` - 刷新物化视图
- `ADMIN SET REPLICA STATUS ...` - 管理副本

### SQL Hint 格式

```sql
SELECT /*+ SET_VAR(query_timeout=600, enable_spill=true) */ ...
```

## ❌ 禁止使用的参数 (不存在或已废弃)

以下参数**不存在**于 StarRocks 中，禁止推荐：
- ❌ `enable_short_key_index` - 不存在！Short Key 是自动的
- ❌ `enable_zone_map_index` - 不存在！Zone Map 是自动的
- ❌ `enable_bitmap_index` - 不存在！用 CREATE INDEX 建索引
- ❌ `enable_async_profile` - 不存在
- ❌ `enable_query_debug_trace` - 不存在
- ❌ `optimize_table` - 不存在！内表用 ADMIN COMPACT
- ❌ 任何你"猜测"可能存在的参数

## ⚠️ 外表限制 (Hive/Iceberg/JDBC 等)

外表**不支持**以下操作，禁止建议：
- ❌ `ALTER TABLE external_table SET ("xxx" = "yyy")` - 外表属性在源端修改
- ❌ `ANALYZE TABLE external_catalog.db.table` - 外表统计信息在源端
- ❌ 任何修改外表分桶/分区的建议
- ❌ `enable_query_cache = true` - Query Cache 不支持外表! 外表用 DataCache!

## 🔄 缓存策略总结

| 缓存类型 | 适用表类型 | 参数 | 说明 |
|---------|-----------|------|------|
| Query Cache | 内表 | `enable_query_cache` | 缓存聚合计算结果 |
| DataCache | 外表 | `enable_scan_datacache` | 缓存远程数据到本地 |
| PageCache | 内表 | 自动 | 缓存磁盘数据页，无需配置 |
"#;

/// Output format specification
const PROMPT_OUTPUT_FORMAT: &str = r#"

## 📤 严格 JSON 输出格式"#;

/// Build the complete dynamic system prompt
pub fn build_system_prompt(request: &RootCauseAnalysisRequest) -> String {
    let mut prompt = String::from(PROMPT_BASE);

    // Add table-type specific guidance
    if let Some(ref profile_data) = request.profile_data {
        prompt.push_str(&build_table_type_prompt(&profile_data.scan_details));
    }

    // Add issue-focused guidance
    prompt.push_str(&build_issue_focused_prompt(&request.rule_diagnostics));

    // Add current session variables
    prompt.push_str(&build_session_vars_prompt(&request.query_summary.session_variables));

    // Add valid parameters reference
    prompt.push_str(PROMPT_VALID_PARAMS);

    // Add output format specification
    prompt.push_str(PROMPT_OUTPUT_FORMAT);

    // Add JSON schema
    prompt.push_str(PROMPT_JSON_FORMAT);

    prompt
}

/// Output format JSON schema (appended to dynamic prompt)
const PROMPT_JSON_FORMAT: &str = r#"

```json
{
  "root_causes": [
    {
      "root_cause_id": "RC001",
      "description": "root cause description based on raw metrics analysis",
      "confidence": 0.85,
      "evidence": ["Profile metric evidence 1", "evidence 2"],
      "symptoms": ["S001", "G003"],
      "is_implicit": false
    }
  ],
  "causal_chains": [
    {
      "chain": ["Root Cause", "->", "Intermediate", "->", "Symptom"],
      "explanation": "Causal analysis based on Profile data"
    }
  ],
  "recommendations": [
    {
      "priority": 1,
      "action": "Brief description of recommended action",
      "expected_improvement": "Quantitative improvement description",
      "sql_example": "Executable SQL or command"
    }
  ],
  "summary": "Overall analysis summary focusing on root causes and optimization direction",
  "hidden_issues": [
    {
      "issue": "Issue not detected by rule engine",
      "suggestion": "Executable solution command"
    }
  ]
}
```

Field descriptions:
- root_cause_id: Format as "RC001", "RC002", etc.
- evidence: MUST reference specific Profile metric values
- symptoms: Related rule IDs
- is_implicit: true if not detected by rule engine
- priority: 1 is highest priority
- sql_example: REQUIRED, executable SQL/command
"#;

/// Legacy static prompt for backward compatibility (minimal)
#[allow(dead_code)]
pub const ROOT_CAUSE_SYSTEM_PROMPT: &str = "You are a StarRocks OLAP database performance expert.";

// ============================================================================
// Request Types
// ============================================================================

/// Root Cause Analysis Request to LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseAnalysisRequest {
    /// Query summary information
    pub query_summary: QuerySummaryForLLM,
    /// Raw profile data for deep analysis (NEW - 原始 Profile 数据)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_data: Option<ProfileDataForLLM>,
    /// Execution plan (simplified for token efficiency)
    pub execution_plan: ExecutionPlanForLLM,
    /// Rule engine diagnostics (for reference, LLM should go deeper)
    pub rule_diagnostics: Vec<DiagnosticForLLM>,
    /// Key performance metrics
    pub key_metrics: KeyMetricsForLLM,
    /// Optional user question for follow-up
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_question: Option<String>,
}

impl LLMAnalysisRequestTrait for RootCauseAnalysisRequest {
    fn scenario(&self) -> LLMScenario {
        LLMScenario::RootCauseAnalysis
    }

    /// Generate dynamic system prompt based on request context
    fn system_prompt(&self) -> String {
        build_system_prompt(self)
    }

    fn cache_key(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.sql_hash().hash(&mut hasher);
        self.profile_hash().hash(&mut hasher);
        format!("rca:{:x}", hasher.finish())
    }

    fn sql_hash(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.query_summary.sql_statement.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    fn profile_hash(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Hash key metrics - use stable values only
        // Don't hash total_time_seconds because it varies per execution!
        // Instead, focus on data volume and structure which are stable
        self.query_summary.scan_bytes.hash(&mut hasher);
        self.query_summary.output_rows.hash(&mut hasher);
        self.rule_diagnostics.len().hash(&mut hasher);
        // Hash query type for better uniqueness
        self.query_summary.query_type.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Query summary for LLM analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySummaryForLLM {
    /// Full SQL statement (NOT truncated - LLM needs complete SQL for analysis)
    pub sql_statement: String,
    /// Query type: SELECT/INSERT/EXPORT/ANALYZE
    pub query_type: String,
    /// Query complexity level: "Simple" | "Medium" | "Complex" | "VeryComplex"
    /// Used for adaptive threshold selection and LLM context
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_complexity: Option<String>,
    /// Total execution time in seconds
    pub total_time_seconds: f64,
    /// Total bytes scanned
    pub scan_bytes: u64,
    /// Output row count
    pub output_rows: u64,
    /// Number of BE nodes
    pub be_count: u32,
    /// Whether spill occurred
    pub has_spill: bool,
    /// Spill details if spill occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spill_bytes: Option<String>,
    /// Non-default session variables (important for analysis)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub session_variables: HashMap<String, String>,
}

// ============================================================================
// Raw Profile Data - NEW: 原始 Profile 数据
// ============================================================================

/// Raw profile data for LLM deep analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDataForLLM {
    /// All operator nodes with their metrics
    pub operators: Vec<OperatorDetailForLLM>,
    /// Cross-node time distribution (for detecting skew)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_distribution: Option<TimeDistributionForLLM>,
    /// Scan node details (tables, partitions, files)
    #[serde(default)]
    pub scan_details: Vec<ScanDetailForLLM>,
    /// Join node details (join type, build/probe stats)
    #[serde(default)]
    pub join_details: Vec<JoinDetailForLLM>,
    /// Aggregation node details
    #[serde(default)]
    pub agg_details: Vec<AggDetailForLLM>,
    /// Exchange (shuffle) details
    #[serde(default)]
    pub exchange_details: Vec<ExchangeDetailForLLM>,
}

/// Detailed operator information with all metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorDetailForLLM {
    /// Operator name (SCAN, JOIN, AGG, etc.)
    pub operator: String,
    /// Plan node ID
    pub plan_node_id: i32,
    /// Execution time percentage
    pub time_pct: f64,
    /// Actual rows processed
    pub rows: u64,
    /// Estimated rows (for cardinality error detection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_rows: Option<u64>,
    /// Memory used in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
    /// All key metrics (raw from profile)
    pub metrics: HashMap<String, String>,
}

/// Time distribution across instances for skew detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDistributionForLLM {
    /// Max time across instances
    pub max_time_ms: f64,
    /// Min time across instances
    pub min_time_ms: f64,
    /// Average time
    pub avg_time_ms: f64,
    /// Skew ratio (max/avg)
    pub skew_ratio: f64,
    /// Per-instance times for top operators
    #[serde(default)]
    pub per_instance: Vec<InstanceTimeForLLM>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceTimeForLLM {
    pub operator: String,
    pub instance_id: i32,
    pub time_ms: f64,
    pub rows: u64,
}

/// Scan operator details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanDetailForLLM {
    pub plan_node_id: i32,
    pub table_name: String,
    /// OlapScan / HdfsScan / ConnectorScan etc.
    pub scan_type: String,
    /// Table storage type: "internal" (StarRocks native), "external" (foreign table)
    /// This is CRITICAL for LLM to give correct suggestions!
    pub table_type: String,
    /// Connector type for external tables: "hive", "iceberg", "hudi", "deltalake", "paimon", "jdbc", "es", "unknown"
    /// For internal tables this is "native"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_type: Option<String>,
    /// Total rows read
    pub rows_read: u64,
    /// Rows after filtering
    pub rows_returned: u64,
    /// Filter ratio
    pub filter_ratio: f64,
    /// Scan ranges (file/tablet count)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_ranges: Option<u64>,
    /// Bytes read
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_read: Option<u64>,
    /// IO wait time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_time_ms: Option<f64>,
    /// Cache hit rate (DataCache for external, PageCache for internal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit_rate: Option<f64>,
    /// Predicates applied
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicates: Option<String>,
    /// Partition pruning info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partitions_scanned: Option<String>,
    /// For external tables: catalog.database.table format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_table_path: Option<String>,
}

/// Join operator details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinDetailForLLM {
    pub plan_node_id: i32,
    /// HASH_JOIN, CROSS_JOIN, etc.
    pub join_type: String,
    /// Build side rows
    pub build_rows: u64,
    /// Probe side rows
    pub probe_rows: u64,
    /// Output rows
    pub output_rows: u64,
    /// Hash table memory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_table_memory: Option<u64>,
    /// Is broadcast join
    #[serde(default)]
    pub is_broadcast: bool,
    /// Runtime filter info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_filter: Option<String>,
}

/// Aggregation operator details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggDetailForLLM {
    pub plan_node_id: i32,
    /// Input rows
    pub input_rows: u64,
    /// Output rows after aggregation
    pub output_rows: u64,
    /// Aggregation ratio (output/input)
    pub agg_ratio: f64,
    /// GROUP BY keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_by_keys: Option<String>,
    /// Hash table memory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_table_memory: Option<u64>,
    /// Is streaming agg
    #[serde(default)]
    pub is_streaming: bool,
}

/// Exchange (shuffle) operator details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeDetailForLLM {
    pub plan_node_id: i32,
    /// SHUFFLE, BROADCAST, GATHER
    pub exchange_type: String,
    /// Data sent bytes
    pub bytes_sent: u64,
    /// Rows sent
    pub rows_sent: u64,
    /// Network time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_time_ms: Option<f64>,
}

/// Simplified execution plan for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlanForLLM {
    /// DAG description in text format
    /// e.g., "SCAN(orders) -> JOIN -> SCAN(customers) -> AGG -> SINK"
    pub dag_description: String,
    /// Hotspot nodes (time_percentage > 15%)
    #[serde(default)]
    pub hotspot_nodes: Vec<HotspotNodeForLLM>,
}

/// Hotspot node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotNodeForLLM {
    /// Operator name, e.g., "HASH_JOIN"
    pub operator: String,
    /// Plan node ID
    pub plan_node_id: i32,
    /// Time percentage (0-100)
    pub time_percentage: f64,
    /// Key metrics relevant to this operator
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub key_metrics: HashMap<String, String>,
    /// Upstream operator names
    #[serde(default)]
    pub upstream_operators: Vec<String>,
}

/// Rule engine diagnostic result for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticForLLM {
    /// Rule ID, e.g., "S001"
    pub rule_id: String,
    /// Severity: Error/Warning/Info
    pub severity: String,
    /// Affected operator
    pub operator: String,
    /// Plan node ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_node_id: Option<i32>,
    /// Diagnostic message
    pub message: String,
    /// Evidence that triggered the rule
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub evidence: HashMap<String, String>,
    /// Threshold metadata for traceability (how the threshold was determined)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_info: Option<ThresholdInfoForLLM>,
}

/// Threshold information for LLM to understand how diagnostics were triggered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdInfoForLLM {
    /// Threshold value used (in appropriate unit, e.g., ms for time)
    pub threshold_value: f64,
    /// Source: "baseline" (adaptive from history) or "default" (static config)
    pub source: String,
    /// If baseline was used, P95 value from historical data (ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_p95_ms: Option<f64>,
    /// Number of samples used to calculate baseline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_count: Option<usize>,
}

/// Key performance metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyMetricsForLLM {
    /// Data skew metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skew_metrics: Option<SkewMetricsForLLM>,
    /// IO metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_metrics: Option<IOMetricsForLLM>,
    /// Memory metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_metrics: Option<MemoryMetricsForLLM>,
    /// Cardinality estimation errors
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cardinality_errors: Vec<CardinalityErrorForLLM>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkewMetricsForLLM {
    pub max_rows: u64,
    pub min_rows: u64,
    pub avg_rows: f64,
    pub skew_ratio: f64,
    pub affected_operator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOMetricsForLLM {
    pub total_bytes_read: u64,
    pub cache_hit_rate: f64,
    pub io_time_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetricsForLLM {
    pub peak_memory_bytes: u64,
    pub spill_bytes: u64,
    pub hash_table_memory: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardinalityErrorForLLM {
    pub operator: String,
    pub estimated_rows: u64,
    pub actual_rows: u64,
    pub error_ratio: f64,
}

// ============================================================================
// Response Types
// ============================================================================

/// Root Cause Analysis Response from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseAnalysisResponse {
    /// Identified root causes
    #[serde(default)]
    pub root_causes: Vec<LLMRootCause>,
    /// Causal chains with explanations
    #[serde(default)]
    pub causal_chains: Vec<LLMCausalChain>,
    /// Prioritized recommendations
    #[serde(default)]
    pub recommendations: Vec<LLMRecommendation>,
    /// Summary in natural language
    #[serde(default)]
    pub summary: String,
    /// Hidden issues not detected by rule engine
    #[serde(default)]
    pub hidden_issues: Vec<LLMHiddenIssue>,
}

impl LLMAnalysisResponseTrait for RootCauseAnalysisResponse {
    fn summary(&self) -> &str {
        &self.summary
    }

    fn confidence(&self) -> Option<f64> {
        if self.root_causes.is_empty() {
            None
        } else {
            Some(
                self.root_causes.iter().map(|r| r.confidence).sum::<f64>()
                    / self.root_causes.len() as f64,
            )
        }
    }
}

/// Root cause identified by LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRootCause {
    /// Unique ID for this root cause
    pub root_cause_id: String,
    /// Description of the root cause
    pub description: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Evidence supporting this conclusion
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Symptom rule IDs caused by this root cause
    #[serde(default)]
    pub symptoms: Vec<String>,
    /// Whether this is an implicit root cause (not detected by rules)
    #[serde(default)]
    pub is_implicit: bool,
}

/// Causal chain with explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMCausalChain {
    /// Chain representation, e.g., ["统计信息过期", "→", "Join顺序不优", "→", "内存过高"]
    pub chain: Vec<String>,
    /// Natural language explanation
    pub explanation: String,
}

/// Recommendation from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRecommendation {
    /// Priority (1 = highest)
    pub priority: u32,
    /// Action to take
    pub action: String,
    /// Expected improvement
    #[serde(default)]
    pub expected_improvement: String,
    /// SQL example if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql_example: Option<String>,
}

/// Hidden issue not detected by rule engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMHiddenIssue {
    /// Issue description
    pub issue: String,
    /// Suggested action
    pub suggestion: String,
}

// ============================================================================
// Builder for RootCauseAnalysisRequest
// ============================================================================

impl RootCauseAnalysisRequest {
    /// Create a new builder
    pub fn builder() -> RootCauseAnalysisRequestBuilder {
        RootCauseAnalysisRequestBuilder::default()
    }
}

#[derive(Default)]
pub struct RootCauseAnalysisRequestBuilder {
    query_summary: Option<QuerySummaryForLLM>,
    profile_data: Option<ProfileDataForLLM>,
    execution_plan: Option<ExecutionPlanForLLM>,
    rule_diagnostics: Vec<DiagnosticForLLM>,
    key_metrics: KeyMetricsForLLM,
    user_question: Option<String>,
}

impl RootCauseAnalysisRequestBuilder {
    pub fn query_summary(mut self, summary: QuerySummaryForLLM) -> Self {
        self.query_summary = Some(summary);
        self
    }

    pub fn profile_data(mut self, data: ProfileDataForLLM) -> Self {
        self.profile_data = Some(data);
        self
    }

    pub fn execution_plan(mut self, plan: ExecutionPlanForLLM) -> Self {
        self.execution_plan = Some(plan);
        self
    }

    pub fn add_diagnostic(mut self, diag: DiagnosticForLLM) -> Self {
        self.rule_diagnostics.push(diag);
        self
    }

    pub fn diagnostics(mut self, diags: Vec<DiagnosticForLLM>) -> Self {
        self.rule_diagnostics = diags;
        self
    }

    pub fn key_metrics(mut self, metrics: KeyMetricsForLLM) -> Self {
        self.key_metrics = metrics;
        self
    }

    pub fn user_question(mut self, question: impl Into<String>) -> Self {
        self.user_question = Some(question.into());
        self
    }

    pub fn build(self) -> Result<RootCauseAnalysisRequest, &'static str> {
        Ok(RootCauseAnalysisRequest {
            query_summary: self.query_summary.ok_or("query_summary is required")?,
            profile_data: self.profile_data,
            execution_plan: self.execution_plan.ok_or("execution_plan is required")?,
            rule_diagnostics: self.rule_diagnostics,
            key_metrics: self.key_metrics,
            user_question: self.user_question,
        })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Determine table type based on CATALOG prefix, not scan operator type!
///
/// StarRocks has two deployment modes:
/// 1. Shared-Nothing (存算一体): internal tables use OLAP_SCAN
/// 2. Shared-Data (存算分离): internal tables use CONNECTOR_SCAN
///
/// Both modes can access external tables (Hive/Iceberg/ES etc.) via catalogs.
///
/// ## The ONLY reliable rule:
/// - `default_catalog` → internal (StarRocks native table)
/// - Any other catalog name → external (foreign table)
///
/// # Arguments
/// * `table_name` - Full table name, may be "catalog.database.table" or "database.table" or just "table"
///
/// # Returns
/// * "internal" - StarRocks native table (in default_catalog)
/// * "external" - External table (any non-default catalog)
pub fn determine_table_type(table_name: &str) -> String {
    match table_name.split('.').collect::<Vec<_>>() {
        parts if parts.len() >= 3 => if parts[0]
            .eq_ignore_ascii_case("default_catalog") { "internal".to_string() } else { "external".to_string() },
        parts if parts.len() == 2 => "internal".to_string(),
        _ => "internal".to_string(),
    }
}

/// Determine external table connector type from Profile metrics
///
/// StarRocks Profile 中各类外表的标识 (from be/src/exec/hdfs_scanner):
/// - **Iceberg**: Has "IcebergV2FormatTimer" section under ORC/Parquet
/// - **Hive**: Has "ORC" or "Parquet" section, but NO Iceberg indicators
/// - **Delta Lake**: Has "DeletionVector" section (Delta uses deletion vectors)
/// - **Hudi**: Has Hudi-specific metrics
/// - **Paimon**: Has Paimon-specific metrics (uses deletion vector too)
/// - **JDBC**: Has JDBC-related metrics
/// - **ES/Elasticsearch**: Has ES-specific metrics
///
/// # Arguments
/// * `metrics` - The unique_metrics map from SCAN node
///
/// # Returns
/// * "iceberg", "hive", "hudi", "paimon", "deltalake", "jdbc", "es", or "unknown"
pub fn determine_connector_type(metrics: &std::collections::HashMap<String, String>) -> String {
    let keys_str = metrics
        .keys()
        .map(|k| k.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let has = |p: &str| keys_str.contains(p);
    match () {
        _ if has("iceberg") || has("deletefilebuild") => "iceberg",
        _ if has("deletionvector") => "deltalake",
        _ if has("hudi") => "hudi",
        _ if has("paimon") => "paimon",
        _ if has("jdbc") => "jdbc",
        _ if has("elasticsearch") || has("_es_") => "es",
        _ if ["orc", "parquet", "stripe", "rowgroup"]
            .iter()
            .any(|p| has(p)) =>
        {
            "hive"
        },
        _ => "unknown",
    }
    .to_string()
}
