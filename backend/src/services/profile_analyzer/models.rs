//! Profile analysis data models
//! 
//! These models represent the structured data extracted from StarRocks query profiles.
//! They are designed to be serializable for API responses and optimized for frontend visualization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    
    // Spill metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_spill_bytes: Option<String>,
    
    // Top time-consuming nodes for quick overview
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_time_consuming_nodes: Option<Vec<TopNode>>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerInfo {
    pub details: HashMap<String, String>,
}

/// Execution phase information including topology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInfo {
    pub topology: String,
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
    
    #[serde(default)]
    pub is_most_consuming: bool,
    #[serde(default)]
    pub is_second_most_consuming: bool,
    
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub unique_metrics: HashMap<String, String>,
}

/// Node type classification for visualization styling
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
    Unknown,
}

impl Default for NodeType {
    fn default() -> Self {
        NodeType::Unknown
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotSeverity {
    Normal,
    Mild,
    Moderate,
    High,
    Severe,
    Critical,
}

impl Default for HotSeverity {
    fn default() -> Self {
        HotSeverity::Normal
    }
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
    #[serde(skip, default)]
    pub node_class: NodeClass,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub children: Vec<i32>,
}

impl TopologyNode {
    /// Infer node class from operator name
    pub fn infer_node_class(name: &str) -> NodeClass {
        match name {
            "EXCHANGE" | "MERGE_EXCHANGE" => NodeClass::ExchangeNode,
            name if name.contains("SCAN") => NodeClass::ScanNode,
            name if name.contains("JOIN") => NodeClass::JoinNode,
            "AGGREGATE" | "AGGREGATION" => NodeClass::AggregationNode,
            "SORT" => NodeClass::SortNode,
            "PROJECT" => NodeClass::ProjectNode,
            "RESULT_SINK" => NodeClass::ResultSink,
            "OLAP_TABLE_SINK" => NodeClass::OlapTableSink,
            _ => NodeClass::Unknown,
        }
    }
}

/// Node classification for topology
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum NodeClass {
    ExchangeNode,
    ScanNode,
    JoinNode,
    AggregationNode,
    SortNode,
    ProjectNode,
    ResultSink,
    OlapTableSink,
    #[default]
    Unknown,
}

// ============================================================================
// Constants
// ============================================================================

pub mod constants {
    /// Time thresholds for performance classification
    pub mod time_thresholds {
        /// Threshold for "most consuming" node (> 30%)
        pub const MOST_CONSUMING_THRESHOLD: f64 = 30.0;
        /// Threshold for "second most consuming" node (> 15%)
        pub const SECOND_CONSUMING_THRESHOLD: f64 = 15.0;
        /// Threshold for metric significance (> 0.3%)
        pub const METRIC_CONSUMING_THRESHOLD: f64 = 0.3;
    }
    
    /// Top N limits
    pub mod top_n {
        pub const TOP_NODES_LIMIT: usize = 3;
    }
    
    /// StarRocks specific constants
    pub mod starrocks {
        pub const MERGED_INFO_PREFIX_MAX: &str = "__MAX_OF_";
        pub const MERGED_INFO_PREFIX_MIN: &str = "__MIN_OF_";
        pub const FINAL_SINK_PSEUDO_PLAN_NODE_ID: i32 = -1;
    }
}
