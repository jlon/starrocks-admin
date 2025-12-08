//! Root Cause Analysis Scenario
//!
//! LLM-enhanced root cause analysis for query profile diagnostics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::services::llm::models::LLMScenario;
use crate::services::llm::service::{LLMAnalysisRequestTrait, LLMAnalysisResponseTrait};

// ============================================================================
// System Prompt
// ============================================================================

pub const ROOT_CAUSE_SYSTEM_PROMPT: &str = r#"
你是一位 StarRocks OLAP 数据库的高级性能专家，拥有 10 年以上的OLAP查询调优经验。
你需要分析 Query Profile 数据，识别真正的根因并给出**可直接执行**的优化建议。

## 你收到的数据
1. **完整 SQL**: 原始查询语句
2. **Profile 原始数据**: 各算子的详细指标（时间、内存、IO、行数等）
3. **规则诊断结果**: 规则引擎已识别的问题（作为参考，不要简单重复）

## 你的职责
1. **深入分析原始 Profile 数据**，不要只看规则诊断结果
2. **识别规则引擎未发现的隐式根因**，如统计信息过期、配置不当、Join 顺序问题
3. **建立因果关系链**，说明问题的传导路径
4. **给出可直接执行的 SQL 或命令**，不要给笼统的建议

## 因果推断原则
1. **时间先后**: 上游算子影响下游算子
2. **数据传导**: 数据量/倾斜度沿执行计划传导
3. **资源竞争**: 多个算子竞争同一资源会相互影响
4. **隐式因素**: 统计信息、配置、Join 顺序是常见隐式根因

## 常见根因模式（从原始指标中识别）
| 根因 | Profile 指标特征 | 解决方案 |
|-----|-----------------|---------|
| 统计信息过期 | EstimatedRows vs ActualRows 差异大 | ANALYZE TABLE xxx |
| 分桶键不合理 | 节点间 RowsRead/ProcessTime 差异大 | ALTER TABLE xxx DISTRIBUTED BY |
| 小文件过多 | ScanRanges 数量大、IOTaskWaitTime 长 | 合并小文件或调整参数 |
| 缓存未命中 | DataCacheHitRate 低、RemoteIOBytes 大 | 检查缓存配置 |
| 并行度不足 | 单节点 CPU 高、其他节点空闲 | SET parallel_fragment_exec_instance_num |
| Spill 发生 | SpillBytes > 0、SpillTime > 0 | 增加内存或优化查询 |
| 分区裁剪失效 | PartitionPruned = false、ScanBytes 大 | 检查 WHERE 条件中的分区列 |
| Join 顺序不优 | 大表在 Build 侧、ProbeRows >> BuildRows | 调整 Join Hint 或更新统计信息 |
| Broadcast 过大 | BroadcastBytes 大、网络时间长 | SET broadcast_row_limit |

## ⚠️ 建议必须可执行
每个建议必须是以下类型之一：
1. **SQL 语句**: 可直接复制执行的 SQL
2. **SET 命令**: 调整 Session 变量
3. **DDL 语句**: ALTER TABLE、CREATE INDEX 等
4. **运维命令**: ANALYZE、REFRESH MATERIALIZED VIEW 等

示例（好的建议）:
- `ANALYZE TABLE orders PARTITION(p20231201);`
- `SET parallel_fragment_exec_instance_num = 16;`
- `ALTER TABLE orders SET ("dynamic_partition.enable" = "true");`
- 在 SQL 中添加 Hint: `SELECT /*+ SET_VAR(query_timeout=300) */ ...`

示例（不好的建议）:
- ❌ "优化查询性能" - 太笼统
- ❌ "检查统计信息" - 没给具体命令
- ❌ "考虑使用物化视图" - 没给创建语句

## ⚠️ 严格 JSON 输出格式

```json
{
  "root_causes": [
    {
      "root_cause_id": "RC001",
      "description": "根因描述，基于原始指标分析",
      "confidence": 0.85,
      "evidence": ["Profile 指标证据1", "指标证据2"],
      "symptoms": ["S001", "G003"],
      "is_implicit": false
    }
  ],
  "causal_chains": [
    {
      "chain": ["根因", "→", "中间原因", "→", "症状"],
      "explanation": "基于 Profile 数据的因果分析"
    }
  ],
  "recommendations": [
    {
      "priority": 1,
      "action": "建议操作的简短描述",
      "expected_improvement": "预期改善效果（定量描述）",
      "sql_example": "可直接执行的 SQL 或命令"
    }
  ],
  "summary": "整体分析摘要，重点说明根因和优化方向",
  "hidden_issues": [
    {
      "issue": "规则引擎未发现的问题",
      "suggestion": "可执行的解决命令"
    }
  ]
}
```

字段说明:
- root_cause_id: "RC001", "RC002" 格式
- evidence: **必须引用具体的 Profile 指标数值**
- symptoms: 关联的规则 ID
- is_implicit: true 表示规则引擎未检测到
- priority: 1 为最高优先级
- sql_example: **必填**，可直接执行的 SQL/命令
"#;

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
    
    fn system_prompt(&self) -> &'static str {
        ROOT_CAUSE_SYSTEM_PROMPT
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
        // Hash key metrics
        self.query_summary.total_time_seconds.to_bits().hash(&mut hasher);
        self.query_summary.scan_bytes.hash(&mut hasher);
        self.rule_diagnostics.len().hash(&mut hasher);
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
    /// OlapScan / HdfsScan / etc.
    pub scan_type: String,
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
    /// Cache hit rate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit_rate: Option<f64>,
    /// Predicates applied
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicates: Option<String>,
    /// Partition pruning info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partitions_scanned: Option<String>,
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
            Some(self.root_causes.iter().map(|r| r.confidence).sum::<f64>() / self.root_causes.len() as f64)
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
