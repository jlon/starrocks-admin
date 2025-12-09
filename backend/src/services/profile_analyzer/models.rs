//! Profile analysis data models
//!
//! These models represent the structured data extracted from StarRocks query profiles.
//! They are designed to be serializable for API responses and optimized for frontend visualization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Session Variable Information
// ============================================================================

/// Information about a non-default session variable
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionVariableInfo {
    /// Default value of the variable
    #[serde(rename = "defaultValue")]
    pub default_value: serde_json::Value,
    /// Actual value set for this session
    #[serde(rename = "actualValue")]
    pub actual_value: serde_json::Value,
}

impl SessionVariableInfo {
    /// Check if the actual value matches a given string value (case-insensitive for booleans)
    pub fn actual_value_is(&self, expected: &str) -> bool {
        match &self.actual_value {
            serde_json::Value::Bool(b) => {
                let expected_lower = expected.to_lowercase();
                (*b && expected_lower == "true") || (!*b && expected_lower == "false")
            },
            serde_json::Value::String(s) => s.eq_ignore_ascii_case(expected),
            serde_json::Value::Number(n) => n.to_string() == expected,
            _ => false,
        }
    }

    /// Get actual value as string
    pub fn actual_value_str(&self) -> String {
        match &self.actual_value {
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => "null".to_string(),
            _ => self.actual_value.to_string(),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse a bytes value string (e.g., "1.5 GB", "100 MB", "1024") to u64 bytes
fn parse_bytes_value(value: &str) -> Option<u64> {
    let value = value.trim();

    // Try to parse as plain number first
    if let Ok(n) = value.parse::<u64>() {
        return Some(n);
    }

    // Parse with unit suffix (e.g., "1.5 GB", "100 MB")
    let parts: Vec<&str> = value.split_whitespace().collect();
    if !parts.is_empty() {
        let num_str = parts[0].replace(",", "");
        let num: f64 = num_str.parse().ok()?;

        let multiplier = if parts.len() >= 2 {
            match parts[1].to_uppercase().as_str() {
                "B" => 1u64,
                "KB" | "K" => 1024,
                "MB" | "M" => 1024 * 1024,
                "GB" | "G" => 1024 * 1024 * 1024,
                "TB" | "T" => 1024 * 1024 * 1024 * 1024,
                _ => 1,
            }
        } else {
            1
        };

        return Some((num * multiplier as f64) as u64);
    }

    None
}

// ============================================================================
// Core Profile Structure
// ============================================================================

/// Complete parsed profile with all analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub summary: ProfileSummary,
    pub planner: PlannerInfo,
    pub execution: ExecutionInfo,
    pub fragments: Vec<Fragment>,
    pub execution_tree: Option<ExecutionTree>,
}

impl Profile {
    /// Get cluster information from the profile
    /// Extracts BE count, instance count, and total scan bytes
    pub fn get_cluster_info(&self) -> ClusterInfo {
        use std::collections::HashSet;

        // Collect unique backend addresses across all fragments
        let mut backends: HashSet<String> = HashSet::new();
        let mut total_instances = 0u32;

        for fragment in &self.fragments {
            for addr in &fragment.backend_addresses {
                backends.insert(addr.clone());
            }
            total_instances += fragment.instance_ids.len() as u32;
        }

        // Extract total scan bytes from execution tree if available
        // Look for BytesRead or CompressedBytesReadTotal in unique_metrics
        let total_scan_bytes = self
            .execution_tree
            .as_ref()
            .map(|tree| {
                tree.nodes
                    .iter()
                    .filter(|n| n.operator_name.to_uppercase().contains("SCAN"))
                    .filter_map(|n| {
                        // Try different metric names for bytes read
                        n.unique_metrics
                            .get("BytesRead")
                            .or_else(|| n.unique_metrics.get("CompressedBytesReadTotal"))
                            .or_else(|| n.unique_metrics.get("RawRowsRead"))
                            .and_then(|v| parse_bytes_value(v))
                    })
                    .sum::<u64>()
            })
            .unwrap_or(0);

        ClusterInfo {
            backend_num: backends.len() as u32,
            instance_num: total_instances,
            total_scan_bytes,
            be_memory_limit: None, // Not available from profile, use default
        }
    }
}

/// Cluster information extracted from profile
#[derive(Debug, Clone, Default)]
pub struct ClusterInfo {
    /// Number of unique backends participating in the query
    pub backend_num: u32,
    /// Total number of fragment instances (reserved for future use)
    #[allow(dead_code)]
    pub instance_num: u32,
    /// Total bytes scanned across all scan operators
    pub total_scan_bytes: u64,
    /// BE memory limit in bytes (optional, for dynamic threshold calculation)
    /// If not available, defaults to 64GB
    pub be_memory_limit: Option<u64>,
}

/// Query summary information extracted from profile header
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileSummary {
    pub query_id: String,
    pub start_time: String,
    pub end_time: String,
    pub total_time: String,
    pub query_state: String,
    pub starrocks_version: String,
    pub sql_statement: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_db: Option<String>,

    pub variables: HashMap<String, String>,

    /// Non-default session variables with their default and actual values
    /// Key: variable name, Value: (default_value, actual_value)
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub non_default_variables: HashMap<String, SessionVariableInfo>,

    // Memory metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_allocated_memory: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_peak_memory: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_sum_memory_usage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_deallocated_memory_usage: Option<String>,

    // Time metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_operator_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_operator_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_execution_wall_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_execution_wall_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_cpu_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_cpu_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_scan_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_scan_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_network_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_cumulative_network_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_peak_schedule_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_peak_schedule_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_deliver_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_deliver_time_ms: Option<f64>,

    // Planner metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planner_total_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planner_total_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collect_profile_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collect_profile_time_ms: Option<f64>,

    // IO metrics (aggregated from scan nodes, for disaggregated storage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_seek_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_seek_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_disk_read_io_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_disk_read_io_time_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_read_io_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_read_io_time_ms: Option<f64>,

    // Aggregated IO statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_raw_rows_read: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes_read: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes_read_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages_count_memory: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages_count_local_disk: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages_count_remote: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_rows: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_bytes_display: Option<String>,

    // Spill metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_spill_bytes: Option<String>,

    // DataCache metrics (for disaggregated storage-compute clusters)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacache_hit_rate: Option<f64>, // 0.0 - 1.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacache_bytes_local: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacache_bytes_remote: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacache_bytes_local_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacache_bytes_remote_display: Option<String>,

    // Top time-consuming nodes for quick overview
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_time_consuming_nodes: Option<Vec<TopNode>>,

    // Profile completeness indicators
    /// Whether the profile is collected asynchronously
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_profile_async: Option<bool>,
    /// Number of retry attempts for profile collection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_times: Option<i32>,
    /// Number of missing BE instances (profile data not received)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_instance_count: Option<i32>,
    /// Total number of BE instances involved in the query
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_instance_count: Option<i32>,
    /// Whether the profile data is complete (no missing instances)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_profile_complete: Option<bool>,
    /// Warning message if profile is incomplete
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_completeness_warning: Option<String>,
}

/// Top time-consuming node for quick performance overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopNode {
    pub rank: u32,
    pub operator_name: String,
    pub plan_node_id: i32,
    pub total_time: String,
    pub time_percentage: f64,
    pub is_most_consuming: bool,
    pub is_second_most_consuming: bool,
}

/// Planner phase information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlannerInfo {
    #[serde(default)]
    pub details: HashMap<String, String>,
    /// HMS (Hive MetaStore) call metrics
    #[serde(default)]
    pub hms_metrics: HMSMetrics,
    /// Total planner time in ms
    #[serde(default)]
    pub total_time_ms: f64,
    /// Optimizer time in ms
    #[serde(default)]
    pub optimizer_time_ms: f64,
}

/// HMS (Hive MetaStore) call metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HMSMetrics {
    /// Time for getDatabase calls (ms)
    pub get_database_ms: f64,
    /// Time for getTable calls (ms)
    pub get_table_ms: f64,
    /// Time for getPartitionsByNames calls (ms)
    pub get_partitions_ms: f64,
    /// Time for getPartitionColumnStats calls (ms)
    pub get_partition_stats_ms: f64,
    /// Time for listPartitionNamesByValue calls (ms)
    pub list_partition_names_ms: f64,
    /// Time for LIST_FS_PARTITIONS calls (ms)
    pub list_fs_partitions_ms: f64,
    /// Total HMS time (sum of all above)
    pub total_hms_time_ms: f64,
}

/// Execution phase information including topology
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionInfo {
    #[serde(default)]
    pub topology: String,
    #[serde(default)]
    pub metrics: HashMap<String, String>,
}

// ============================================================================
// Fragment and Pipeline Structure
// ============================================================================

/// A fragment represents a distributed execution unit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    pub id: String,
    pub backend_addresses: Vec<String>,
    pub instance_ids: Vec<String>,
    pub pipelines: Vec<Pipeline>,
}

/// A pipeline within a fragment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub id: String,
    pub metrics: HashMap<String, String>,
    pub operators: Vec<Operator>,
}

/// An operator within a pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operator {
    pub name: String,
    pub plan_node_id: Option<String>,
    pub operator_id: Option<String>,
    pub common_metrics: HashMap<String, String>,
    pub unique_metrics: HashMap<String, String>,
    pub children: Vec<Operator>,
}

// ============================================================================
// Execution Tree Structure (for DAG visualization)
// ============================================================================

/// The execution tree for DAG visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTree {
    pub root: ExecutionTreeNode,
    pub nodes: Vec<ExecutionTreeNode>,
}

/// A node in the execution tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTreeNode {
    pub id: String,
    pub operator_name: String,
    pub node_type: NodeType,
    pub plan_node_id: Option<i32>,
    pub parent_plan_node_id: Option<i32>,
    pub metrics: OperatorMetrics,
    pub children: Vec<String>,
    pub depth: usize,
    pub is_hotspot: bool,
    pub hotspot_severity: HotSeverity,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_percentage: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<u64>,

    #[serde(default)]
    pub is_most_consuming: bool,
    #[serde(default)]
    pub is_second_most_consuming: bool,

    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub unique_metrics: HashMap<String, String>,

    /// Whether this node has diagnostic issues (for UI warning indicator)
    #[serde(default)]
    pub has_diagnostic: bool,
    /// List of diagnostic rule IDs associated with this node
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub diagnostic_ids: Vec<String>,
}

/// Node type classification for visualization styling
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum NodeType {
    OlapScan,
    ConnectorScan,
    HashJoin,
    Aggregate,
    Limit,
    ExchangeSink,
    ExchangeSource,
    ResultSink,
    ChunkAccumulate,
    Sort,
    Project,
    TableFunction,
    OlapTableSink,
    #[default]
    Unknown,
}

// ============================================================================
// Operator Metrics
// ============================================================================

/// Common metrics for all operators
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OperatorMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_total_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_total_time_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_total_time_min: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_total_time_max: Option<u64>,

    pub push_chunk_num: Option<u64>,
    pub push_row_num: Option<u64>,
    pub pull_chunk_num: Option<u64>,
    pub pull_row_num: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_total_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_total_time_min: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_total_time_max: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_total_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_total_time_min: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_total_time_max: Option<u64>,

    pub memory_usage: Option<u64>,
    pub output_chunk_bytes: Option<u64>,

    pub specialized: OperatorSpecializedMetrics,
}

/// Specialized metrics for different operator types
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum OperatorSpecializedMetrics {
    #[default]
    None,
    ConnectorScan(ScanMetrics),
    OlapScan(ScanMetrics),
    ExchangeSink(ExchangeSinkMetrics),
    Join(JoinMetrics),
    Aggregate(AggregateMetrics),
    ResultSink(ResultSinkMetrics),
}

/// Scan operator specific metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanMetrics {
    pub table: String,
    pub rollup: String,
    pub shared_scan: bool,
    pub scan_time_ns: Option<u64>,
    pub io_time_ns: Option<u64>,
    pub bytes_read: Option<u64>,
    pub rows_read: Option<u64>,
}

/// Exchange sink operator metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExchangeSinkMetrics {
    pub dest_fragment_ids: Vec<String>,
    pub dest_be_addresses: Vec<String>,
    pub part_type: String,
    pub bytes_sent: Option<u64>,
    pub network_time_ns: Option<u64>,
}

/// Join operator metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JoinMetrics {
    pub join_type: String,
    pub build_rows: Option<u64>,
    pub probe_rows: Option<u64>,
    pub runtime_filter_num: Option<u64>,
}

/// Aggregate operator metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateMetrics {
    pub agg_mode: String,
    pub chunk_by_chunk: bool,
    pub input_rows: Option<u64>,
    pub agg_function_time_ns: Option<u64>,
}

/// Result sink operator metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResultSinkMetrics {
    pub sink_type: String,
    pub append_chunk_time_ns: Option<u64>,
    pub result_send_time_ns: Option<u64>,
}

// ============================================================================
// Analysis Results
// ============================================================================

/// Hotspot severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HotSeverity {
    #[default]
    Normal,
    Mild,
    Moderate,
    High,
    Severe,
    Critical,
}

/// A detected performance hotspot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotSpot {
    pub node_path: String,
    pub severity: HotSeverity,
    pub issue_type: String,
    pub description: String,
    pub suggestions: Vec<String>,
}

/// Complete analysis response for API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileAnalysisResponse {
    pub hotspots: Vec<HotSpot>,
    pub conclusion: String,
    pub suggestions: Vec<String>,
    pub performance_score: f64,
    pub execution_tree: Option<ExecutionTree>,
    pub summary: Option<ProfileSummary>,
    /// Rule-based diagnostics with parameter suggestions (all diagnostics)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DiagnosticResult>,
    /// Aggregated diagnostics by rule_id for overview display
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aggregated_diagnostics: Vec<AggregatedDiagnostic>,
    /// Node-level diagnostics mapping (plan_node_id -> diagnostics)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub node_diagnostics: std::collections::HashMap<i32, Vec<DiagnosticResult>>,
    /// Raw profile content for display in PROFILE tab
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_content: Option<String>,
    /// Fragment and Pipeline information for node detail view
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fragments: Vec<Fragment>,
    /// Root cause analysis result (rule-based, without LLM)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cause_analysis: Option<super::analyzer::RootCauseAnalysis>,
    /// LLM-enhanced analysis result (async loaded, may be None initially)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_analysis: Option<LLMEnhancedAnalysis>,
}

/// LLM-enhanced analysis result (merged with rule engine)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LLMEnhancedAnalysis {
    /// Whether LLM analysis is available
    pub available: bool,
    /// LLM analysis status: "pending" | "completed" | "failed" | "disabled"
    pub status: String,
    /// Root causes (may include implicit ones not detected by rules)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_causes: Vec<MergedRootCause>,
    /// Causal chains with explanations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causal_chains: Vec<LLMCausalChain>,
    /// Merged recommendations (rule + LLM, deduplicated)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merged_recommendations: Vec<MergedRecommendation>,
    /// Natural language summary from LLM
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    /// Hidden issues detected by LLM only
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_issues: Vec<LLMHiddenIssue>,
    /// Whether this result was loaded from cache
    #[serde(default)]
    pub from_cache: bool,
    /// LLM analysis elapsed time in milliseconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_time_ms: Option<u64>,
}

/// Merged root cause (from rule engine and/or LLM)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedRootCause {
    /// Unique identifier (e.g., "RC001" from LLM or "rule_S001" from rules)
    pub id: String,
    /// Related rule IDs (if detected by rules)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_rule_ids: Vec<String>,
    /// Description of the root cause
    pub description: String,
    /// Is this an implicit root cause (not detected by rules)?
    #[serde(default)]
    pub is_implicit: bool,
    /// Confidence score (1.0 for rule-based, 0.0-1.0 for LLM)
    pub confidence: f64,
    /// Source: "rule" | "llm" | "both"
    pub source: String,
    /// Evidence supporting this conclusion
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// Symptom rule IDs caused by this root cause
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symptoms: Vec<String>,
}

/// Causal chain with explanation from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMCausalChain {
    /// Chain representation, e.g., ["统计信息过期", "→", "Join顺序不优", "→", "内存过高"]
    pub chain: Vec<String>,
    /// Natural language explanation
    pub explanation: String,
}

/// Merged recommendation from rule engine and LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedRecommendation {
    /// Priority (1 = highest)
    pub priority: u32,
    /// Action description
    pub action: String,
    /// Expected improvement
    #[serde(default)]
    pub expected_improvement: String,
    /// SQL example (if applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql_example: Option<String>,
    /// Source: "rule" | "llm" | "both"
    pub source: String,
    /// Related root cause IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_root_causes: Vec<String>,
    /// Is this a root cause fix (vs symptom fix)?
    #[serde(default)]
    pub is_root_cause_fix: bool,
}

/// Hidden issue detected by LLM only
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMHiddenIssue {
    /// Issue description
    pub issue: String,
    /// Suggested action
    pub suggestion: String,
}

/// Aggregated diagnostic for overview display
/// Groups multiple diagnostics of the same rule_id together
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedDiagnostic {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: String,
    /// Aggregated summary message
    pub message: String,
    /// Detailed explanation
    pub reason: String,
    /// List of affected node paths
    pub affected_nodes: Vec<String>,
    /// Number of affected nodes
    pub node_count: usize,
    /// Merged suggestions (deduplicated)
    pub suggestions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_suggestions: Vec<ParameterTuningSuggestion>,
}

/// Diagnostic result from rule engine
///
/// Structure follows Aliyun EMR StarRocks diagnostic standard:
/// - message: 诊断结果概要说明 (Summary of the issue)
/// - reason: 详细诊断原因说明 (Detailed explanation of why this happens)
/// - suggestions: 建议措施 (Recommended actions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: String,
    pub node_path: String,
    /// Plan node ID for associating with execution tree node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_node_id: Option<i32>,
    /// Summary of the diagnostic issue (诊断结果概要)
    pub message: String,
    /// Detailed explanation of why this issue occurs (详细诊断原因)
    pub reason: String,
    /// Recommended actions to fix the issue (建议措施)
    pub suggestions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_suggestions: Vec<ParameterTuningSuggestion>,
    /// Threshold metadata for traceability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_metadata: Option<ThresholdMetadataResult>,
}

/// Threshold metadata for traceability (serializable version)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdMetadataResult {
    /// Threshold value used (e.g., 10000.0 for 10s)
    pub threshold_value: f64,
    /// Threshold source: "baseline" | "default" | "config"
    pub threshold_source: String,
    /// Baseline P95 value if baseline was used (ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_p95_ms: Option<f64>,
    /// Baseline sample count if baseline was used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_sample_count: Option<usize>,
}

/// Parameter tuning suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterTuningSuggestion {
    pub name: String,
    pub param_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    pub recommended: String,
    pub command: String,
    /// Human-readable description of what this parameter does
    #[serde(default)]
    pub description: String,
    /// Expected impact of changing this parameter
    #[serde(default)]
    pub impact: String,
}

// ============================================================================
// Topology Graph (for parsing)
// ============================================================================

/// Topology graph parsed from execution info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyGraph {
    pub root_id: i32,
    pub nodes: Vec<TopologyNode>,
}

/// A node in the topology graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub children: Vec<i32>,
}

// ============================================================================
// Constants
// ============================================================================

pub mod constants {
    /// Time thresholds for performance classification
    /// Aligned with StarRocks ExplainAnalyzer.java:1546-1550
    pub mod time_thresholds {
        /// Threshold for "most consuming" node (> 30%)
        pub const MOST_CONSUMING_THRESHOLD: f64 = 30.0;
        /// Threshold for "second most consuming" node (> 15%)
        pub const SECOND_CONSUMING_THRESHOLD: f64 = 15.0;
    }
}
