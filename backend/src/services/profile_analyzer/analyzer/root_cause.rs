//! Root Cause Analysis Engine (v5.0)
//!
//! Implements multi-dimensional causal analysis without LLM:
//! 1. Intra-Node Causality - same node, multiple diagnostics
//! 2. Inter-Node Propagation - DAG-based propagation
//! 3. Multiple Root Causes - identify all independent root causes
//! 4. Causal Graph - build and visualize causal relationships

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::rules::Diagnostic;

// ============================================================================
// Root Cause Analysis Result Types
// ============================================================================

/// Complete root cause analysis result
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RootCauseAnalysis {
    /// Identified root causes (sorted by impact)
    pub root_causes: Vec<RootCause>,
    /// Causal chains explaining how root causes lead to symptoms
    pub causal_chains: Vec<CausalChain>,
    /// Natural language summary
    pub summary: String,
    /// Total number of diagnostics analyzed
    pub total_diagnostics: usize,
}

/// A single root cause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCause {
    /// Unique ID (e.g., "RC001")
    pub id: String,
    /// Related diagnostic rule IDs
    pub diagnostic_ids: Vec<String>,
    /// Description of the root cause
    pub description: String,
    /// Impact percentage (0-100)
    pub impact_percentage: f64,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Node paths affected
    pub affected_nodes: Vec<String>,
    /// Evidence supporting this conclusion
    pub evidence: Vec<String>,
    /// Symptoms caused by this root cause
    pub symptoms: Vec<String>,
    /// Suggested actions
    pub suggestions: Vec<String>,
}

/// A causal chain from root cause to symptom
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalChain {
    /// Chain elements: ["Root Cause", "→", "Intermediate", "→", "Symptom"]
    pub chain: Vec<String>,
    /// Explanation of this causal relationship
    pub explanation: String,
    /// Confidence score
    pub confidence: f64,
}

// ============================================================================
// Intra-Node Causality Rules
// ============================================================================

/// Rule defining causality within the same node
struct IntraNodeRule {
    /// Cause diagnostic IDs (any of these)
    causes: &'static [&'static str],
    /// Effect diagnostic ID
    effect: &'static str,
    /// Description of the causal relationship
    description: &'static str,
}

/// Predefined intra-node causality rules based on v5.0 design
///
/// Rule Categories:
/// - S: SCAN node rules (S001-S016)
/// - J: JOIN node rules (J001-J011)
/// - A: AGGREGATE node rules (A001-A006)
/// - Q: Query-level rules (Q001-Q009)
/// - G: General rules (G001-G003)
/// - E: EXCHANGE node rules (E001-E003)
/// - T: SORT node rules (T001-T005)
/// - W: WINDOW node rules (W001)
/// - I: SINK/Insert rules (I001-I003)
/// - F: Fragment rules (F001-F003)
const INTRA_NODE_RULES: &[IntraNodeRule] = &[
    // ========================================================================
    // SCAN Node Causality (16 rules covered)
    // ========================================================================
    IntraNodeRule {
        causes: &["S016", "S006"], // Small files (S016), Rowset fragmentation (S006)
        effect: "S007",            // IO bottleneck
        description: "小文件/碎片化导致IO瓶颈",
    },
    IntraNodeRule {
        causes: &["S017"], // ORC Stripe fragmentation
        effect: "S007",    // IO bottleneck
        description: "Stripe碎片化导致IO瓶颈",
    },
    IntraNodeRule {
        causes: &["S017"], // ORC Stripe fragmentation
        effect: "S018",    // IO wait time
        description: "Stripe碎片化导致IO等待时间长",
    },
    IntraNodeRule {
        causes: &["S017"], // ORC Stripe fragmentation
        effect: "G001",    // Time consuming node
        description: "Stripe碎片化导致SCAN耗时长",
    },
    IntraNodeRule {
        causes: &["S018"], // IO wait time
        effect: "G001",    // Time consuming node
        description: "IO等待时间长导致SCAN耗时长",
    },
    IntraNodeRule {
        causes: &["S018"], // IO wait time
        effect: "Q004",    // Low CPU utilization
        description: "IO等待导致CPU空闲",
    },
    IntraNodeRule {
        causes: &["S009"], // Low cache hit ratio
        effect: "S007",    // IO bottleneck
        description: "缓存命中率低导致IO瓶颈",
    },
    IntraNodeRule {
        causes: &["S009"], // Low cache hit ratio
        effect: "G001",    // Time consuming node (SCAN)
        description: "缓存命中率低导致SCAN耗时长",
    },
    IntraNodeRule {
        causes: &["S008", "S012", "S013"], // ZoneMap/Bitmap/BloomFilter ineffective
        effect: "S003",                    // Poor filter effectiveness
        description: "索引未生效导致过滤效果差",
    },
    IntraNodeRule {
        causes: &["S001"], // Data skew in SCAN
        effect: "G003",    // Execution time skew
        description: "数据倾斜导致执行时间倾斜",
    },
    IntraNodeRule {
        causes: &["S003"], // Poor filter effectiveness
        effect: "S002",    // Full table scan
        description: "过滤效果差导致全表扫描",
    },
    IntraNodeRule {
        causes: &["S010"], // Large compressed ratio
        effect: "S007",    // IO bottleneck (decompression overhead)
        description: "高压缩率导致解压开销大",
    },
    IntraNodeRule {
        causes: &["S014"], // Too many segments
        effect: "S007",    // IO bottleneck
        description: "Segment过多导致IO瓶颈",
    },
    // ========================================================================
    // JOIN Node Causality (11 rules covered)
    // ========================================================================
    IntraNodeRule {
        causes: &["J002"], // Suboptimal join order
        effect: "J001",    // Hash table too large
        description: "Join顺序不优导致Hash表过大",
    },
    IntraNodeRule {
        causes: &["J005"], // Broadcast table too large
        effect: "E002",    // Network bottleneck
        description: "Broadcast表过大导致网络瓶颈",
    },
    IntraNodeRule {
        causes: &["J003"], // Join probe rows skew
        effect: "G003",    // Execution time skew
        description: "Join探测端数据倾斜导致时间倾斜",
    },
    IntraNodeRule {
        causes: &["J001"], // Hash table too large
        effect: "Q003",    // Spill to disk
        description: "Hash表过大导致内存溢出到磁盘",
    },
    IntraNodeRule {
        causes: &["J006"], // Missing runtime filter
        effect: "S003",    // Poor filter effectiveness on probe side
        description: "缺少Runtime Filter导致探测端过滤差",
    },
    IntraNodeRule {
        causes: &["J007"], // Runtime filter not pushed down
        effect: "S003",    // Poor filter effectiveness
        description: "Runtime Filter未下推导致过滤效果差",
    },
    IntraNodeRule {
        causes: &["J008"], // Join condition not optimal
        effect: "J001",    // Large hash table
        description: "Join条件不优导致Hash表过大",
    },
    IntraNodeRule {
        causes: &["J009"], // Cross join detected
        effect: "G002",    // High CPU utilization
        description: "笛卡尔积导致CPU使用率高",
    },
    IntraNodeRule {
        causes: &["J010"], // Join type not optimal
        effect: "E002",    // Network bottleneck (wrong distribution)
        description: "Join类型不优导致数据传输过多",
    },
    // ========================================================================
    // AGGREGATE Node Causality (6 rules covered)
    // ========================================================================
    IntraNodeRule {
        causes: &["A001"], // Aggregation skew
        effect: "Q003",    // Spill occurred
        description: "聚合倾斜导致内存溢出",
    },
    IntraNodeRule {
        causes: &["A001"], // Aggregation skew
        effect: "G003",    // Execution time skew
        description: "聚合倾斜导致执行时间倾斜",
    },
    IntraNodeRule {
        causes: &["A003"], // Too many distinct keys
        effect: "A002",    // Large hash table
        description: "Distinct键过多导致Hash表过大",
    },
    IntraNodeRule {
        causes: &["A002"], // Large hash table
        effect: "Q003",    // Spill to disk
        description: "聚合Hash表过大导致溢出",
    },
    IntraNodeRule {
        causes: &["A004"], // Missing streaming aggregation
        effect: "A002",    // Large hash table
        description: "未使用流式聚合导致内存占用高",
    },
    IntraNodeRule {
        causes: &["A005"], // High aggregation cardinality
        effect: "G002",    // High CPU utilization
        description: "聚合基数过高导致CPU占用高",
    },
    // ========================================================================
    // SORT Node Causality (5 rules covered)
    // ========================================================================
    IntraNodeRule {
        causes: &["T001"], // Sort data too large
        effect: "Q003",    // Spill to disk
        description: "排序数据量过大导致溢出",
    },
    IntraNodeRule {
        causes: &["T002"], // TopN not optimized
        effect: "T001",    // Large sort data
        description: "TopN未优化导致排序数据量大",
    },
    IntraNodeRule {
        causes: &["T003"], // Sort without limit
        effect: "T001",    // Large sort data
        description: "全量排序导致数据量过大",
    },
    IntraNodeRule {
        causes: &["T004"], // Multiple sort keys
        effect: "G002",    // High CPU utilization
        description: "多排序键导致CPU占用高",
    },
    // ========================================================================
    // EXCHANGE Node Causality (3 rules covered)
    // ========================================================================
    IntraNodeRule {
        causes: &["E001"], // Large data shuffle
        effect: "E002",    // Network bottleneck
        description: "Shuffle数据量大导致网络瓶颈",
    },
    IntraNodeRule {
        causes: &["E003"], // Partition skew
        effect: "G003",    // Execution time skew
        description: "分区倾斜导致执行时间倾斜",
    },
    // ========================================================================
    // Query-Level Causality - CRITICAL: Link symptoms to root causes
    // ========================================================================
    // Spill causes timeout
    IntraNodeRule {
        causes: &["Q003"], // Spill to disk
        effect: "Q001",    // Query timeout
        description: "磁盘溢出导致查询变慢可能超时",
    },
    // Resource waiting causes timeout
    IntraNodeRule {
        causes: &["Q006"], // Resource queue waiting
        effect: "Q001",    // Query timeout
        description: "资源队列等待可能导致超时",
    },
    // IO/Wait bottleneck causes low CPU
    IntraNodeRule {
        causes: &["S007"], // IO bottleneck
        effect: "Q004",    // Low CPU utilization
        description: "IO瓶颈导致CPU利用率低",
    },
    IntraNodeRule {
        causes: &["S009"], // Low cache hit
        effect: "Q004",    // Low CPU (waiting for remote IO)
        description: "缓存未命中导致等待远程IO，CPU空闲",
    },
    IntraNodeRule {
        causes: &["E002"], // Network bottleneck
        effect: "Q004",    // Low CPU (waiting for network)
        description: "网络等待导致CPU利用率低",
    },
    // ========================================================================
    // G001/G001b/G002 Causality - Most consuming nodes cause query symptoms
    // ========================================================================
    // SCAN bottleneck causes scan time ratio high
    IntraNodeRule {
        causes: &["G001", "G001b"], // Most/Second consuming node (usually SCAN)
        effect: "Q005",             // Scan time ratio high
        description: "扫描算子耗时长导致扫描时间占比高",
    },
    // High memory nodes cause query peak memory
    IntraNodeRule {
        causes: &["G002"], // High memory usage node
        effect: "Q002",    // Query peak memory high
        description: "算子内存高导致查询峰值内存高",
    },
    // Slow execution causes query timeout
    IntraNodeRule {
        causes: &["G001"], // Most consuming node
        effect: "Q001",    // Query timeout
        description: "耗时算子导致查询超时",
    },
    // ========================================================================
    // Memory Hierarchy Causality - Specific memory → General memory
    // ========================================================================
    // Join HashTable memory causes node high memory
    IntraNodeRule {
        causes: &["J003"], // Join HashTable memory high
        effect: "G002",    // Node memory high
        description: "Join HashTable内存高导致节点内存高",
    },
    // Aggregation HashTable causes node high memory
    IntraNodeRule {
        causes: &["A002"], // Aggregation HashTable memory high
        effect: "G002",    // Node memory high
        description: "聚合HashTable内存高导致节点内存高",
    },
    // Hash collision causes HashTable memory high
    IntraNodeRule {
        causes: &["J005"], // Hash collision
        effect: "J003",    // HashTable memory high
        description: "Hash碰撞导致HashTable内存增大",
    },
    // Broadcast too large causes memory issues
    IntraNodeRule {
        causes: &["J011"], // Broadcast build side too large
        effect: "J003",    // HashTable memory high
        description: "Broadcast端数据量大导致HashTable内存高",
    },
    IntraNodeRule {
        causes: &["J011"], // Broadcast build side too large
        effect: "G002",    // Node memory high
        description: "Broadcast端数据量大导致节点内存高",
    },
    // ========================================================================
    // Network/Scheduling Causality
    // ========================================================================
    // Large shuffle causes scheduling overhead
    IntraNodeRule {
        causes: &["E001"], // Large shuffle data
        effect: "Q008",    // Scheduling overhead high
        description: "Shuffle数据量大导致调度开销增加",
    },
    // Scheduling overhead causes timeout
    IntraNodeRule {
        causes: &["Q008"], // Scheduling overhead
        effect: "Q001",    // Query timeout
        description: "调度开销大导致查询变慢",
    },
    // ========================================================================
    // PROJECT/LIMIT Causality
    // ========================================================================
    IntraNodeRule {
        causes: &["P001"], // Complex expression in project
        effect: "G002",    // High CPU utilization
        description: "复杂表达式计算导致CPU占用高",
    },
    // ========================================================================
    // SINK Causality (I001-I003 are terminal - effects, not causes)
    // ========================================================================
    IntraNodeRule {
        causes: &["E002"], // Network bottleneck
        effect: "I001",    // Insert slow
        description: "网络瓶颈导致数据导入慢",
    },
    IntraNodeRule {
        causes: &["A001"], // Aggregation skew
        effect: "I002",    // Insert skew
        description: "聚合倾斜导致导入数据倾斜",
    },
];

// ============================================================================
// Inter-Node Propagation Rules
// ============================================================================

/// Propagation mode between nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropagationMode {
    /// Data volume propagates: upstream data → downstream processing time
    DataVolume,
    /// Skew propagates: upstream skew → downstream skew
    Skew,
    /// Memory pressure propagates: sibling memory usage → spill
    Memory,
    /// IO wait propagates: upstream IO → downstream stall
    IoWait,
}

/// Rule for inter-node propagation
struct InterNodeRule {
    /// Upstream diagnostic pattern
    upstream: &'static str,
    /// Downstream diagnostic pattern
    downstream: &'static str,
    /// Propagation mode (reserved for future use)
    #[allow(dead_code)]
    mode: PropagationMode,
    /// Description
    description: &'static str,
}

/// Inter-node propagation rules - how issues propagate along the DAG
///
/// Propagation Modes:
/// - DataVolume: Data size propagates downstream
/// - Skew: Data skew propagates downstream
/// - Memory: Memory pressure causes spill
/// - IoWait: IO wait propagates to downstream stall
const INTER_NODE_RULES: &[InterNodeRule] = &[
    // ========================================================================
    // Skew Propagation (upstream skew → downstream skew)
    // ========================================================================
    InterNodeRule {
        upstream: "S001",   // SCAN data skew
        downstream: "G003", // Execution time skew
        mode: PropagationMode::Skew,
        description: "SCAN数据倾斜传导到下游执行时间倾斜",
    },
    InterNodeRule {
        upstream: "S001",   // SCAN data skew
        downstream: "J003", // Join probe rows skew
        mode: PropagationMode::Skew,
        description: "SCAN数据倾斜传导到Join探测端",
    },
    InterNodeRule {
        upstream: "S001",   // SCAN data skew
        downstream: "A001", // Aggregation skew
        mode: PropagationMode::Skew,
        description: "SCAN数据倾斜传导到聚合倾斜",
    },
    InterNodeRule {
        upstream: "S001",   // SCAN data skew
        downstream: "E003", // Exchange partition skew
        mode: PropagationMode::Skew,
        description: "SCAN数据倾斜传导到Shuffle分区倾斜",
    },
    InterNodeRule {
        upstream: "J003",   // Join probe skew
        downstream: "A001", // Aggregation skew
        mode: PropagationMode::Skew,
        description: "Join倾斜传导到聚合倾斜",
    },
    InterNodeRule {
        upstream: "E003",   // Exchange partition skew
        downstream: "G003", // Execution time skew
        mode: PropagationMode::Skew,
        description: "Shuffle倾斜传导到执行时间倾斜",
    },
    // ========================================================================
    // Data Volume Propagation (large data → downstream processing burden)
    // ========================================================================
    InterNodeRule {
        upstream: "S003",   // Poor filter effectiveness
        downstream: "J001", // Hash table too large
        mode: PropagationMode::DataVolume,
        description: "过滤效果差导致下游Join数据量大",
    },
    InterNodeRule {
        upstream: "S003",   // Poor filter effectiveness
        downstream: "A002", // Aggregation hash table large
        mode: PropagationMode::DataVolume,
        description: "过滤效果差导致聚合处理数据量大",
    },
    InterNodeRule {
        upstream: "S003",   // Poor filter effectiveness
        downstream: "E001", // Large shuffle data
        mode: PropagationMode::DataVolume,
        description: "过滤效果差导致Shuffle数据量大",
    },
    InterNodeRule {
        upstream: "S003",   // Poor filter effectiveness
        downstream: "T001", // Sort data too large
        mode: PropagationMode::DataVolume,
        description: "过滤效果差导致排序数据量大",
    },
    InterNodeRule {
        upstream: "S002",   // Full table scan
        downstream: "J001", // Hash table too large
        mode: PropagationMode::DataVolume,
        description: "全表扫描导致Join数据量大",
    },
    InterNodeRule {
        upstream: "S002",   // Full table scan
        downstream: "E001", // Large shuffle data
        mode: PropagationMode::DataVolume,
        description: "全表扫描导致Shuffle数据量大",
    },
    InterNodeRule {
        upstream: "J001",   // Large join hash table
        downstream: "A002", // Large aggregation hash table
        mode: PropagationMode::DataVolume,
        description: "Join输出数据量大导致聚合数据量大",
    },
    InterNodeRule {
        upstream: "J009",   // Cross join (cartesian product)
        downstream: "A002", // Large aggregation hash table
        mode: PropagationMode::DataVolume,
        description: "笛卡尔积导致下游数据爆炸",
    },
    InterNodeRule {
        upstream: "J009",   // Cross join
        downstream: "T001", // Large sort data
        mode: PropagationMode::DataVolume,
        description: "笛卡尔积导致排序数据量爆炸",
    },
    // ========================================================================
    // Memory Propagation (memory pressure → spill)
    // ========================================================================
    InterNodeRule {
        upstream: "J001",   // Hash table large
        downstream: "Q003", // Spill
        mode: PropagationMode::Memory,
        description: "Join内存占用高导致触发Spill",
    },
    InterNodeRule {
        upstream: "A002",   // Aggregation hash table large
        downstream: "Q003", // Spill
        mode: PropagationMode::Memory,
        description: "聚合内存占用高导致触发Spill",
    },
    InterNodeRule {
        upstream: "T001",   // Large sort data
        downstream: "Q003", // Spill
        mode: PropagationMode::Memory,
        description: "排序数据量大导致触发Spill",
    },
    InterNodeRule {
        upstream: "W001",   // Window function memory
        downstream: "Q003", // Spill
        mode: PropagationMode::Memory,
        description: "窗口函数内存占用导致Spill",
    },
    // ========================================================================
    // IO Wait Propagation (IO bottleneck → downstream stall)
    // Note: These are conditionally applied - E002 should only cause G001
    // when they are on the SAME or related EXCHANGE nodes, not globally.
    // We remove E002→G001 as it causes incorrect causality for non-EXCHANGE G001.
    // ========================================================================
    InterNodeRule {
        upstream: "S007",   // IO bottleneck (SCAN level)
        downstream: "G001", // Time consuming node (if same SCAN node)
        mode: PropagationMode::IoWait,
        description: "IO瓶颈导致SCAN节点耗时长",
    },
    InterNodeRule {
        upstream: "S017",   // ORC Stripe fragmentation
        downstream: "G001", // Time consuming node
        mode: PropagationMode::IoWait,
        description: "Stripe碎片化导致SCAN节点耗时长",
    },
    InterNodeRule {
        upstream: "S017",   // Stripe fragmentation
        downstream: "S018", // IO wait time
        mode: PropagationMode::IoWait,
        description: "Stripe碎片化导致IO等待",
    },
    InterNodeRule {
        upstream: "S018",   // IO wait time
        downstream: "G001", // Time consuming node
        mode: PropagationMode::IoWait,
        description: "IO等待导致节点耗时长",
    },
    InterNodeRule {
        upstream: "S018",   // IO wait time
        downstream: "Q004", // Low CPU
        mode: PropagationMode::IoWait,
        description: "IO等待导致CPU利用率低",
    },
    InterNodeRule {
        upstream: "Q003",   // Spill to disk
        downstream: "G001", // Time consuming node
        mode: PropagationMode::IoWait,
        description: "磁盘溢出导致节点耗时长",
    },
    // ========================================================================
    // Query-Level Symptom Propagation (node issues → query symptoms)
    // These link operator-level issues to query-level aggregated symptoms
    // ========================================================================
    InterNodeRule {
        upstream: "G001",   // Most consuming node (SCAN)
        downstream: "Q005", // Scan time ratio high (Query level)
        mode: PropagationMode::DataVolume,
        description: "SCAN耗时长导致扫描时间占比高",
    },
    InterNodeRule {
        upstream: "G001b",  // Second consuming node
        downstream: "Q005", // Scan time ratio high
        mode: PropagationMode::DataVolume,
        description: "次耗时SCAN导致扫描时间占比高",
    },
    InterNodeRule {
        upstream: "G002",   // High memory node
        downstream: "Q002", // Query peak memory high
        mode: PropagationMode::Memory,
        description: "节点内存高导致查询峰值内存高",
    },
    InterNodeRule {
        upstream: "G001",   // Most consuming node
        downstream: "Q001", // Query timeout
        mode: PropagationMode::IoWait,
        description: "耗时算子导致查询超时",
    },
    InterNodeRule {
        upstream: "J003",   // Join HashTable memory
        downstream: "Q002", // Query peak memory
        mode: PropagationMode::Memory,
        description: "Join内存高导致查询峰值内存高",
    },
    InterNodeRule {
        upstream: "A002",   // Aggregation memory
        downstream: "Q002", // Query peak memory
        mode: PropagationMode::Memory,
        description: "聚合内存高导致查询峰值内存高",
    },
    InterNodeRule {
        upstream: "E001",   // Large shuffle
        downstream: "Q008", // Scheduling overhead
        mode: PropagationMode::DataVolume,
        description: "Shuffle数据量大导致调度开销增加",
    },
    InterNodeRule {
        upstream: "Q008",   // Scheduling overhead
        downstream: "Q001", // Query timeout
        mode: PropagationMode::IoWait,
        description: "调度开销大导致查询超时",
    },
    InterNodeRule {
        upstream: "J011",   // Broadcast too large
        downstream: "G002", // High memory node
        mode: PropagationMode::Memory,
        description: "Broadcast数据大导致节点内存高",
    },
    InterNodeRule {
        upstream: "J005",   // Hash collision
        downstream: "G002", // High memory node
        mode: PropagationMode::Memory,
        description: "Hash碰撞导致节点内存增大",
    },
    // ========================================================================
    // Additional Query-Level Causality
    // ========================================================================
    // High memory pressure can cause timeout
    InterNodeRule {
        upstream: "Q002",   // Query peak memory high
        downstream: "Q001", // Query timeout
        mode: PropagationMode::Memory,
        description: "内存压力过大可能导致查询超时",
    },
    // Aggregation as root cause for G001 (when AGG is most consuming)
    InterNodeRule {
        upstream: "A002",   // Aggregation HashTable large
        downstream: "G001", // Most consuming node (AGG)
        mode: PropagationMode::Memory,
        description: "聚合HashTable过大导致聚合算子耗时长",
    },
    // Join as root cause for G001 (when JOIN is most consuming)
    InterNodeRule {
        upstream: "J003",   // Join HashTable memory
        downstream: "G001", // Most consuming node (JOIN)
        mode: PropagationMode::Memory,
        description: "Join HashTable过大导致Join算子耗时长",
    },
    InterNodeRule {
        upstream: "J001",   // Join hash table too large
        downstream: "G001", // Most consuming node
        mode: PropagationMode::Memory,
        description: "Join数据量大导致Join算子耗时长",
    },
    // E001 large shuffle can cause G001 for EXCHANGE nodes
    InterNodeRule {
        upstream: "E001",   // Large shuffle data
        downstream: "G001", // Most consuming node (EXCHANGE)
        mode: PropagationMode::DataVolume,
        description: "Shuffle数据量大导致EXCHANGE算子耗时长",
    },
    // E002 network ratio causes E001 which then causes G001
    InterNodeRule {
        upstream: "E001",   // Large shuffle (cause)
        downstream: "E002", // Network ratio high (effect)
        mode: PropagationMode::IoWait,
        description: "大Shuffle导致网络时间占比高",
    },
    // ========================================================================
    // IO/Cache → CPU Utilization (cross-node symptom propagation)
    // ========================================================================
    InterNodeRule {
        upstream: "S009",   // Low cache hit
        downstream: "Q004", // Low CPU utilization
        mode: PropagationMode::IoWait,
        description: "缓存未命中导致等待远程IO，CPU空闲",
    },
    InterNodeRule {
        upstream: "S007",   // IO bottleneck
        downstream: "Q004", // Low CPU utilization
        mode: PropagationMode::IoWait,
        description: "IO瓶颈导致CPU利用率低",
    },
    InterNodeRule {
        upstream: "E002",   // Network time ratio high
        downstream: "Q004", // Low CPU (waiting for network)
        mode: PropagationMode::IoWait,
        description: "网络等待导致CPU利用率低",
    },
];

// ============================================================================
// Root Cause Analyzer
// ============================================================================

/// Root cause analyzer implementing v5.0 design
pub struct RootCauseAnalyzer;

impl RootCauseAnalyzer {
    /// Analyze diagnostics and identify root causes
    pub fn analyze(diagnostics: &[Diagnostic]) -> RootCauseAnalysis {
        if diagnostics.is_empty() {
            return RootCauseAnalysis::default();
        }

        // Build diagnostic lookup
        let diag_map = Self::build_diagnostic_map(diagnostics);

        // Step 1: Find intra-node causal relationships
        let intra_edges = Self::find_intra_node_causality(diagnostics);

        // Step 2: Find inter-node causal relationships
        let inter_edges = Self::find_inter_node_propagation(diagnostics);

        // Step 3: Build causal graph
        let all_edges: Vec<(String, String, String)> = intra_edges
            .into_iter()
            .chain(inter_edges)
            .collect();

        // Step 4: Find root causes (nodes with no incoming edges)
        let root_causes = Self::identify_root_causes(diagnostics, &all_edges, &diag_map);

        // Step 5: Build causal chains
        let causal_chains = Self::build_causal_chains(&root_causes, &all_edges, &diag_map);

        // Step 6: Generate summary
        let summary = Self::generate_summary(&root_causes);

        RootCauseAnalysis {
            root_causes,
            causal_chains,
            summary,
            total_diagnostics: diagnostics.len(),
        }
    }

    /// Build a map of rule_id -> diagnostics for quick lookup
    fn build_diagnostic_map(diagnostics: &[Diagnostic]) -> HashMap<String, Vec<&Diagnostic>> {
        let mut map: HashMap<String, Vec<&Diagnostic>> = HashMap::new();
        for diag in diagnostics {
            map.entry(diag.rule_id.clone()).or_default().push(diag);
        }
        map
    }

    /// Find intra-node causal relationships
    fn find_intra_node_causality(diagnostics: &[Diagnostic]) -> Vec<(String, String, String)> {
        let mut edges = Vec::new();

        // Group diagnostics by node path
        let mut by_node: HashMap<&str, Vec<&Diagnostic>> = HashMap::new();
        for diag in diagnostics {
            by_node.entry(&diag.node_path).or_default().push(diag);
        }

        // Check each node for intra-node causality
        for (_node_path, node_diags) in by_node {
            let rule_ids: HashSet<&str> = node_diags.iter().map(|d| d.rule_id.as_str()).collect();

            for rule in INTRA_NODE_RULES {
                // Check if any cause is present
                let has_cause = rule.causes.iter().any(|c| rule_ids.contains(*c));
                // Check if effect is present
                let has_effect = rule_ids.contains(rule.effect);

                if has_cause && has_effect {
                    // Find the specific cause that's present
                    for cause in rule.causes {
                        if rule_ids.contains(*cause) {
                            edges.push((
                                cause.to_string(),
                                rule.effect.to_string(),
                                rule.description.to_string(),
                            ));
                        }
                    }
                }
            }
        }

        edges
    }

    /// Find inter-node causal relationships based on DAG
    fn find_inter_node_propagation(diagnostics: &[Diagnostic]) -> Vec<(String, String, String)> {
        let mut edges = Vec::new();
        let rule_ids: HashSet<&str> = diagnostics.iter().map(|d| d.rule_id.as_str()).collect();

        for rule in INTER_NODE_RULES {
            if rule_ids.contains(rule.upstream) && rule_ids.contains(rule.downstream) {
                edges.push((
                    rule.upstream.to_string(),
                    rule.downstream.to_string(),
                    rule.description.to_string(),
                ));
            }
        }

        edges
    }

    /// Identify root causes (diagnostics with no incoming causal edges)
    fn identify_root_causes(
        diagnostics: &[Diagnostic],
        edges: &[(String, String, String)],
        diag_map: &HashMap<String, Vec<&Diagnostic>>,
    ) -> Vec<RootCause> {
        // Find all rule_ids that have incoming edges (are effects)
        let effects: HashSet<&str> = edges.iter().map(|(_, effect, _)| effect.as_str()).collect();

        // Note: causes set could be used for more advanced analysis (e.g., cycle detection)
        let _causes: HashSet<&str> = edges.iter().map(|(cause, _, _)| cause.as_str()).collect();

        // Root causes: present in diagnostics, not an effect of another, OR has no incoming edge
        let mut root_cause_ids: HashSet<&str> = HashSet::new();

        // All unique rule_ids in diagnostics
        let all_ids: HashSet<&str> = diagnostics.iter().map(|d| d.rule_id.as_str()).collect();

        for id in &all_ids {
            // If this ID is not caused by anything else in our graph, it's a root cause
            if !effects.contains(id) {
                root_cause_ids.insert(id);
            }
        }

        // If no root causes found, treat the highest severity diagnostics as root causes
        if root_cause_ids.is_empty() {
            for diag in diagnostics.iter().take(3) {
                root_cause_ids.insert(&diag.rule_id);
            }
        }

        // Build RootCause objects
        let mut root_causes: Vec<RootCause> = Vec::new();
        let mut rc_counter = 0;

        for rule_id in root_cause_ids {
            if let Some(diags) = diag_map.get(rule_id) {
                rc_counter += 1;
                let first_diag = diags[0];

                // Find symptoms caused by this root cause
                let symptoms: Vec<String> = edges
                    .iter()
                    .filter(|(cause, _, _)| cause == rule_id)
                    .map(|(_, effect, _)| effect.clone())
                    .collect();

                // Calculate impact based on severity and symptom count
                let base_impact = match first_diag.severity {
                    super::rules::RuleSeverity::Error => 40.0,
                    super::rules::RuleSeverity::Warning => 25.0,
                    super::rules::RuleSeverity::Info => 15.0,
                };
                let symptom_bonus = symptoms.len() as f64 * 10.0;
                let impact = (base_impact + symptom_bonus).min(100.0);

                // Merge suggestions intelligently when multiple nodes have the same issue
                let suggestions = Self::merge_suggestions_for_root_cause(diags);

                root_causes.push(RootCause {
                    id: format!("RC{:03}", rc_counter),
                    diagnostic_ids: vec![rule_id.to_string()],
                    description: first_diag.message.clone(),
                    impact_percentage: impact,
                    confidence: 1.0, // Rule-based = 100% confidence
                    affected_nodes: diags.iter().map(|d| d.node_path.clone()).collect(),
                    evidence: vec![first_diag.reason.clone()],
                    symptoms,
                    suggestions,
                });
            }
        }

        // Sort by impact (descending)
        root_causes.sort_by(|a, b| {
            b.impact_percentage
                .partial_cmp(&a.impact_percentage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        root_causes
    }

    /// Merge suggestions from multiple diagnostics of the same rule
    /// Consolidates similar suggestions (e.g., different table names) into one
    fn merge_suggestions_for_root_cause(diags: &[&Diagnostic]) -> Vec<String> {
        if diags.len() == 1 {
            return diags[0].suggestions.clone();
        }

        // Extract table names from reason fields
        let tables: Vec<String> = diags
            .iter()
            .filter_map(|d| Self::extract_table_name(&d.reason))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let first_sug = diags[0]
            .suggestions
            .first()
            .map(|s| s.as_str())
            .unwrap_or("");

        // Check if this is a file fragmentation issue
        if first_sug.contains("外表小文件合并方案") || first_sug.contains("Compaction 合并碎片")
        {
            let tables_str = if tables.is_empty() {
                format!("{} 个表", diags.len())
            } else if tables.len() <= 3 {
                tables.join(", ")
            } else {
                format!("{} 等 {} 个表", tables[0], tables.len())
            };

            return if first_sug.contains("外表小文件合并方案") {
                vec![format!(
                    "外表小文件合并 (涉及: {}): ①ALTER TABLE <table> PARTITION(...) CONCATENATE; \
                     ②INSERT OVERWRITE TABLE <table> SELECT * FROM <table>; \
                     ③Spark: df.repartition(N).saveAsTable('<table>'); \
                     ④SET connector_io_tasks_per_scan_operator=64",
                    tables_str
                )]
            } else {
                vec![format!("执行 Compaction: ALTER TABLE <{}> COMPACT", tables_str)]
            };
        }

        // Default: dedupe and limit to 3
        let mut seen = HashSet::new();
        diags
            .iter()
            .flat_map(|d| d.suggestions.iter())
            .filter(|s| seen.insert(s.as_str()))
            .take(3)
            .cloned()
            .collect()
    }

    /// Extract table name from reason like "外表「table_name」的 ORC..."
    fn extract_table_name(reason: &str) -> Option<String> {
        let start = reason.find('「')?;
        let end = reason.find('」')?;
        (end > start).then(|| reason[start + 3..end].to_string())
    }

    /// Build causal chains from root causes to symptoms
    /// Deduplicates by rule_name chain (not rule_id) to avoid visual duplicates
    fn build_causal_chains(
        root_causes: &[RootCause],
        edges: &[(String, String, String)],
        diag_map: &HashMap<String, Vec<&Diagnostic>>,
    ) -> Vec<CausalChain> {
        let mut chains = Vec::new();
        let mut seen_rule_id_chains: HashSet<String> = HashSet::new();
        let mut seen_name_chains: HashSet<String> = HashSet::new();

        for rc in root_causes {
            for diag_id in &rc.diagnostic_ids {
                let paths = Self::find_paths_from(diag_id, edges, 3);

                for path in paths {
                    if path.len() < 2 {
                        continue;
                    }

                    // Deduplicate by rule_id path first
                    let id_key = path.join("->");
                    if seen_rule_id_chains.contains(&id_key) {
                        continue;
                    }
                    seen_rule_id_chains.insert(id_key);

                    // Build human-readable chain and deduplicate by name
                    let names: Vec<String> = path
                        .iter()
                        .map(|id| {
                            diag_map
                                .get(id)
                                .and_then(|d| d.first())
                                .map(|d| d.rule_name.clone())
                                .unwrap_or_else(|| id.clone())
                        })
                        .collect();

                    let name_key = names.join("->");
                    if seen_name_chains.contains(&name_key) {
                        continue;
                    }
                    seen_name_chains.insert(name_key);

                    // Build display chain with arrows
                    let mut chain = Vec::new();
                    let mut explanations = Vec::new();

                    for (i, (node_id, name)) in path.iter().zip(names.iter()).enumerate() {
                        chain.push(name.clone());
                        if i < path.len() - 1 {
                            chain.push("→".to_string());
                            if let Some((_, _, desc)) = edges
                                .iter()
                                .find(|(c, e, _)| c == node_id && e == &path[i + 1])
                            {
                                explanations.push(desc.clone());
                            }
                        }
                    }

                    let explanation = if explanations.is_empty() {
                        format!(
                            "{} 导致 {}",
                            names.first().unwrap_or(&path[0]),
                            names.last().unwrap_or(&path[0])
                        )
                    } else {
                        explanations.join("; ")
                    };

                    chains.push(CausalChain { chain, explanation, confidence: 1.0 });
                }
            }
        }

        // Sort: shorter chains first, then alphabetically
        chains.sort_by(|a, b| {
            a.chain
                .len()
                .cmp(&b.chain.len())
                .then_with(|| a.chain.join("").cmp(&b.chain.join("")))
        });

        // Limit output
        chains.truncate(10);
        chains
    }

    /// Find all paths from a starting node using BFS
    fn find_paths_from(
        start: &str,
        edges: &[(String, String, String)],
        max_depth: usize,
    ) -> Vec<Vec<String>> {
        let mut paths = Vec::new();
        let mut queue: Vec<(Vec<String>, usize)> = vec![(vec![start.to_string()], 0)];

        while let Some((path, depth)) = queue.pop() {
            if depth >= max_depth {
                if path.len() > 1 {
                    paths.push(path);
                }
                continue;
            }

            let current = path.last().unwrap();
            let mut found_next = false;

            for (cause, effect, _) in edges {
                if cause == current && !path.contains(effect) {
                    found_next = true;
                    let mut new_path = path.clone();
                    new_path.push(effect.clone());
                    queue.push((new_path, depth + 1));
                }
            }

            if !found_next && path.len() > 1 {
                paths.push(path);
            }
        }

        paths
    }

    /// Generate a natural language summary
    fn generate_summary(root_causes: &[RootCause]) -> String {
        if root_causes.is_empty() {
            return "未发现明显的性能问题根因".to_string();
        }

        if root_causes.len() == 1 {
            let rc = &root_causes[0];
            if rc.symptoms.is_empty() {
                format!("发现 1 个根因: {}", rc.description)
            } else {
                format!(
                    "发现 1 个根因: {}，导致了 {} 个下游问题",
                    rc.description,
                    rc.symptoms.len()
                )
            }
        } else {
            let top_causes: Vec<&str> = root_causes
                .iter()
                .take(3)
                .map(|rc| rc.diagnostic_ids.first().map(|s| s.as_str()).unwrap_or(""))
                .collect();

            format!(
                "发现 {} 个独立根因，主要包括: {}。建议按优先级依次解决",
                root_causes.len(),
                top_causes.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::rules::RuleSeverity;
    use super::*;

    fn make_diag(rule_id: &str, node_path: &str, severity: RuleSeverity) -> Diagnostic {
        Diagnostic {
            rule_id: rule_id.to_string(),
            rule_name: format!("Rule {}", rule_id),
            severity,
            node_path: node_path.to_string(),
            plan_node_id: None,
            message: format!("Message for {}", rule_id),
            reason: format!("Reason for {}", rule_id),
            suggestions: vec![format!("Fix {}", rule_id)],
            parameter_suggestions: vec![],
            threshold_metadata: None,
        }
    }

    #[test]
    fn test_intra_node_causality() {
        // S016 (small files) + S007 (IO bottleneck) in same SCAN node
        let diagnostics = vec![
            make_diag("S016", "Fragment_1/Pipeline_0/SCAN", RuleSeverity::Warning),
            make_diag("S007", "Fragment_1/Pipeline_0/SCAN", RuleSeverity::Error),
        ];

        let result = RootCauseAnalyzer::analyze(&diagnostics);

        // S016 should be identified as root cause, S007 as symptom
        assert!(!result.root_causes.is_empty());
        assert!(
            result
                .root_causes
                .iter()
                .any(|rc| rc.diagnostic_ids.contains(&"S016".to_string()))
        );
        assert!(result.causal_chains.iter().any(|cc| {
            cc.chain
                .iter()
                .any(|s| s.contains("S016") || s.contains("小文件"))
        }));
    }

    #[test]
    fn test_inter_node_propagation() {
        // S001 (data skew) in SCAN -> G003 (time skew) in JOIN
        let diagnostics = vec![
            make_diag("S001", "Fragment_1/Pipeline_0/SCAN", RuleSeverity::Warning),
            make_diag("G003", "Fragment_1/Pipeline_1/JOIN", RuleSeverity::Warning),
        ];

        let result = RootCauseAnalyzer::analyze(&diagnostics);

        // S001 should be root cause, G003 should be symptom
        assert!(!result.root_causes.is_empty());
        assert!(
            result
                .root_causes
                .iter()
                .any(|rc| rc.diagnostic_ids.contains(&"S001".to_string()))
        );
    }

    #[test]
    fn test_multiple_root_causes() {
        // Two independent issues: S016 (small files) and J002 (bad join order)
        let diagnostics = vec![
            make_diag("S016", "Fragment_1/Pipeline_0/SCAN_1", RuleSeverity::Warning),
            make_diag("J002", "Fragment_1/Pipeline_1/JOIN", RuleSeverity::Warning),
        ];

        let result = RootCauseAnalyzer::analyze(&diagnostics);

        // Both should be identified as root causes (no causal relationship between them)
        assert_eq!(result.root_causes.len(), 2);
    }
}
