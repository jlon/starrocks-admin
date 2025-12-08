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
你需要分析 Query Profile 数据，识别真正的根因并给出可执行的优化建议。

## 你的职责
1. 分析规则引擎已检测到的诊断结果
2. 识别规则引擎未能发现的隐式根因
3. 建立完整的因果关系链
4. 给出优先级排序的、可执行的优化建议

## 因果推断原则
1. **时间先后**: 原因必须发生在结果之前（上游算子 → 下游算子）
2. **数据传导**: 数据量/倾斜度会沿着执行计划传导
3. **资源竞争**: 多个算子竞争同一资源可能导致性能下降
4. **隐式因素**: 统计信息过期、配置不当等是常见的隐式根因

## 常见根因模式（优先考虑）
- **统计信息过期** → 基数估算错误 → Join 顺序不优 / Broadcast 选择错误
- **分桶键不合理** → 数据倾斜 → 执行时间倾斜
- **小文件过多** → IO 请求数高 → IO 瓶颈
- **缓存命中低** → 远程读取多 → IO 延迟高
- **并行度不足** → CPU 利用率低 → 执行时间长
- **内存不足** → Spill 到磁盘 → 性能下降

## 输出要求
1. **置信度**: 基于证据充分性给出 0.0-1.0 的置信度
2. **建议**: 必须是可执行的具体操作，优先提供 SQL 示例
3. **优先级**: 按影响程度排序，优先解决根因
4. **语言**: 使用中文

## 输出格式
严格按照以下 JSON 格式输出：
{
  "root_causes": [...],
  "causal_chains": [...],
  "recommendations": [...],
  "summary": "...",
  "hidden_issues": [...]
}
"#;

// ============================================================================
// Request Types
// ============================================================================

/// Root Cause Analysis Request to LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseAnalysisRequest {
    /// Query summary information
    pub query_summary: QuerySummaryForLLM,
    /// Execution plan (simplified for token efficiency)
    pub execution_plan: ExecutionPlanForLLM,
    /// Rule engine diagnostics
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
    /// SQL statement (truncated if > 2000 chars)
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
    /// Non-default session variables
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub session_variables: HashMap<String, String>,
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
            execution_plan: self.execution_plan.ok_or("execution_plan is required")?,
            rule_diagnostics: self.rule_diagnostics,
            key_metrics: self.key_metrics,
            user_question: self.user_question,
        })
    }
}
