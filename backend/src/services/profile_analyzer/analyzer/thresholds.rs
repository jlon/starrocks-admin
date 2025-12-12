//! Dynamic Thresholds for Profile Diagnostics
//!
//! Implements dynamic threshold calculation based on:
//! - Cluster configuration (BE count, memory)
//! - Query type (SELECT, INSERT, EXPORT, etc.)
//! - Storage type (S3, HDFS, local)
//! - Historical baseline from audit logs
//! - Query complexity (simple/medium/complex/very complex)
//!
//! Reference: profile-diagnostic-system-review.md Section 4

use super::baseline::{PerformanceBaseline, QueryComplexity};
use crate::services::profile_analyzer::models::ClusterInfo;

// ============================================================================
// Query Type Detection
// ============================================================================

/// Query type classification for threshold adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Normal SELECT query (OLAP)
    Select,
    /// INSERT INTO SELECT (ETL)
    Insert,
    /// EXPORT query
    Export,
    /// ANALYZE TABLE
    Analyze,
    /// CREATE TABLE AS SELECT
    Ctas,
    /// Broker Load / Routine Load
    Load,
    /// Unknown query type
    Unknown,
}

impl QueryType {
    /// Detect query type from SQL statement
    pub fn from_sql(sql: &str) -> Self {
        let sql = sql.trim().to_uppercase();

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

    /// Get execution time threshold in milliseconds
    pub fn get_time_threshold_ms(&self) -> f64 {
        match self {
            QueryType::Select => 10_000.0,   // OLAP: 10s
            QueryType::Insert => 300_000.0,  // ETL: 5min
            QueryType::Export => 600_000.0,  // Export: 10min
            QueryType::Analyze => 600_000.0, // Analyze: 10min
            QueryType::Ctas => 300_000.0,    // CTAS: 5min
            QueryType::Load => 1_800_000.0,  // Load: 30min
            QueryType::Unknown => 60_000.0,  // Default: 1min
        }
    }

    /// Check if a rule should be skipped for this query type
    pub fn should_skip_rule(&self, rule_id: &str) -> bool {
        match self {
            // INSERT/CTAS: Skip Q001 (different time threshold), skip filter effectiveness rules
            QueryType::Insert | QueryType::Ctas => {
                matches!(rule_id, "Q001" | "S003")
            },
            // EXPORT: Skip IO-related warnings (IO is expected to be high)
            QueryType::Export => {
                matches!(rule_id, "S007" | "Q005")
            },
            // ANALYZE: Skip scan-related warnings
            QueryType::Analyze => {
                matches!(rule_id, "S003" | "S007" | "Q005" | "Q006")
            },
            // LOAD: Only check sink-related rules
            QueryType::Load => {
                !rule_id.starts_with('I') // Only I001-I003 apply
            },
            _ => false,
        }
    }
}

// ============================================================================
// Dynamic Thresholds
// ============================================================================

/// Dynamic threshold calculator based on cluster info and query type
#[derive(Debug, Clone)]
pub struct DynamicThresholds {
    pub cluster_info: ClusterInfo,
    pub query_type: QueryType,
    pub query_complexity: QueryComplexity,
    /// Historical baseline data (optional)
    pub baseline: Option<PerformanceBaseline>,
}

impl DynamicThresholds {
    /// Create a new threshold calculator
    pub fn new(
        cluster_info: ClusterInfo,
        query_type: QueryType,
        query_complexity: QueryComplexity,
    ) -> Self {
        Self { cluster_info, query_type, query_complexity, baseline: None }
    }

    /// Create with default cluster info
    pub fn with_defaults(query_type: QueryType) -> Self {
        Self {
            cluster_info: ClusterInfo::default(),
            query_type,
            query_complexity: QueryComplexity::Medium,
            baseline: None,
        }
    }

    /// Create with historical baseline
    pub fn with_baseline(
        cluster_info: ClusterInfo,
        query_type: QueryType,
        query_complexity: QueryComplexity,
        baseline: PerformanceBaseline,
    ) -> Self {
        Self { cluster_info, query_type, query_complexity, baseline: Some(baseline) }
    }

    /// Detect query complexity from SQL
    pub fn detect_complexity(sql: &str) -> QueryComplexity {
        QueryComplexity::from_sql(sql)
    }

    // ========================================================================
    // Memory Thresholds
    // ========================================================================

    /// Get memory threshold for operator peak memory (G002)
    /// Returns threshold in bytes
    ///
    /// Logic: 10% of BE memory, with min 1GB and max 10GB
    pub fn get_operator_memory_threshold(&self) -> u64 {
        let be_memory = self
            .cluster_info
            .be_memory_limit
            .unwrap_or(64 * 1024 * 1024 * 1024); // Default 64GB

        let threshold = (be_memory as f64 * 0.1) as u64;

        // Clamp between 1GB and 10GB
        const MIN_THRESHOLD: u64 = 1024 * 1024 * 1024; // 1GB
        const MAX_THRESHOLD: u64 = 10 * 1024 * 1024 * 1024; // 10GB

        threshold.clamp(MIN_THRESHOLD, MAX_THRESHOLD)
    }

    /// Get memory threshold for HashTable (J003, A002)
    /// Returns threshold in bytes
    ///
    /// Logic: 5% of BE memory, with min 512MB and max 5GB
    pub fn get_hash_table_memory_threshold(&self) -> u64 {
        let be_memory = self
            .cluster_info
            .be_memory_limit
            .unwrap_or(64 * 1024 * 1024 * 1024);

        let threshold = (be_memory as f64 * 0.05) as u64;

        const MIN_THRESHOLD: u64 = 512 * 1024 * 1024; // 512MB
        const MAX_THRESHOLD: u64 = 5 * 1024 * 1024 * 1024; // 5GB

        threshold.clamp(MIN_THRESHOLD, MAX_THRESHOLD)
    }

    // ========================================================================
    // Time Thresholds
    // ========================================================================

    /// Get execution time threshold for Q001 (query too long)
    /// Returns threshold in milliseconds
    ///
    /// Strategy:
    /// 1. If historical baseline available: use P95 + 2*std_dev
    /// 2. Otherwise: use query type baseline * complexity factor
    #[allow(dead_code)] // Used by QueryType directly in Q001
    pub fn get_query_time_threshold_ms(&self) -> f64 {
        // Priority 1: Use historical baseline if available
        if let Some(baseline) = &self.baseline {
            let adaptive_threshold = baseline.stats.p95_ms + 2.0 * baseline.stats.std_dev_ms;

            // Ensure reasonable minimum
            let min_threshold = self.get_min_threshold_by_complexity();
            return adaptive_threshold.max(min_threshold);
        }

        // Priority 2: Use query type + complexity factor
        let base = self.query_type.get_time_threshold_ms();
        let complexity_factor = self.get_complexity_factor();

        base * complexity_factor
    }

    /// Get complexity factor for threshold adjustment
    fn get_complexity_factor(&self) -> f64 {
        match self.query_complexity {
            QueryComplexity::Simple => 0.5,      // Simple: 50% of base
            QueryComplexity::Medium => 1.0,      // Medium: 100% of base
            QueryComplexity::Complex => 2.0,     // Complex: 200% of base
            QueryComplexity::VeryComplex => 3.0, // Very complex: 300% of base
        }
    }

    /// Get minimum threshold by complexity (to avoid too strict)
    fn get_min_threshold_by_complexity(&self) -> f64 {
        match self.query_complexity {
            QueryComplexity::Simple => 5_000.0,       // 5s
            QueryComplexity::Medium => 10_000.0,      // 10s
            QueryComplexity::Complex => 30_000.0,     // 30s
            QueryComplexity::VeryComplex => 60_000.0, // 1min
        }
    }

    /// Get minimum diagnosis time threshold
    /// Queries faster than this won't be diagnosed
    /// Returns threshold in seconds
    pub fn get_min_diagnosis_time_seconds(&self) -> f64 {
        match self.query_type {
            // For ETL/Load, even fast queries might have issues
            QueryType::Insert | QueryType::Load | QueryType::Ctas => 0.5,
            // For OLAP, skip very fast queries
            _ => 1.0,
        }
    }

    // ========================================================================
    // Skew Thresholds
    // ========================================================================

    /// Get data skew threshold (S001, J006, A001, G003)
    /// Returns max/avg ratio threshold
    ///
    /// Strategy:
    /// 1. If historical baseline available: learn from P99/P50 ratio
    /// 2. Otherwise: use cluster size-based threshold
    pub fn get_skew_threshold(&self) -> f64 {
        let parallelism = self.cluster_info.backend_num;

        // Base threshold from cluster size
        let base = match parallelism {
            p if p > 32 => 3.5, // Large cluster: allow more skew
            p if p > 16 => 3.0,
            p if p > 8 => 2.5,
            _ => 2.0, // Small cluster: stricter
        };

        // If baseline available, adjust based on historical variance
        if let Some(baseline) = &self.baseline {
            let historical_ratio = if baseline.stats.p50_ms > 0.0 {
                baseline.stats.p99_ms / baseline.stats.p50_ms
            } else {
                2.0
            };

            // If historical data shows high variance, allow slightly more skew
            // Increase base by 20% of (historical_ratio - 2.0), capped at 1.0
            let adjustment = ((historical_ratio - 2.0) * 0.2).clamp(0.0, 1.0);
            base + adjustment
        } else {
            base
        }
    }

    // ========================================================================
    // Cache Thresholds
    // ========================================================================

    /// Get cache hit rate threshold (S009)
    /// Returns minimum acceptable hit rate (0.0 - 1.0)
    ///
    /// Logic: Disaggregated storage needs higher hit rate
    pub fn get_cache_hit_threshold(&self) -> f64 {
        // Check if this is disaggregated storage based on scan bytes from remote
        // For now, use a conservative default
        0.5 // 50% hit rate as warning threshold
    }

    // ========================================================================
    // Small File Thresholds
    // ========================================================================

    /// Get small file size threshold based on storage type
    /// Returns threshold in bytes
    pub fn get_small_file_threshold(&self, storage_type: &str) -> u64 {
        match storage_type.to_uppercase().as_str() {
            "S3" | "OSS" | "COS" | "GCS" => 128 * 1024 * 1024, // Object storage: 128MB
            "HDFS" => 64 * 1024 * 1024,                        // HDFS: 64MB (block size)
            "LOCAL" => 32 * 1024 * 1024,                       // Local: 32MB
            _ => 64 * 1024 * 1024,                             // Default: 64MB
        }
    }

    /// Get minimum file count to trigger small file detection
    pub fn get_min_file_count(&self, storage_type: &str) -> u64 {
        match storage_type.to_uppercase().as_str() {
            "LOCAL" => 200,
            _ => 500,
        }
    }

    // ========================================================================
    // Row Count Thresholds
    // ========================================================================

    /// Get minimum row count for skew detection (S001)
    pub fn get_min_rows_for_skew(&self) -> f64 {
        100_000.0 // 100k rows
    }

    /// Get minimum row count for filter effectiveness (S003)
    pub fn get_min_rows_for_filter(&self) -> f64 {
        100_000.0 // 100k rows
    }

    /// Get minimum row count for join explosion (J001)
    pub fn get_min_rows_for_join(&self) -> f64 {
        10_000.0 // 10k rows
    }
}

impl Default for DynamicThresholds {
    fn default() -> Self {
        Self {
            cluster_info: ClusterInfo::default(),
            query_type: QueryType::Unknown,
            query_complexity: QueryComplexity::Medium,
            baseline: None,
        }
    }
}

// ============================================================================
// Threshold Constants (for backward compatibility)
// ============================================================================

/// Default thresholds module for rules that don't have access to DynamicThresholds
/// These constants are kept for backward compatibility and as fallback values
pub mod defaults {
    /// Minimum diagnosis time in seconds (P0.1)
    /// Note: Now using DynamicThresholds::get_min_diagnosis_time_seconds() which varies by query type
    #[allow(dead_code)]
    pub const MIN_DIAGNOSIS_TIME_SECONDS: f64 = 1.0;

    /// Minimum operator time in milliseconds for G001/G001b (P0.2)
    pub const MIN_OPERATOR_TIME_MS: f64 = 500.0;

    /// Time percentage threshold for most consuming (G001)
    pub const MOST_CONSUMING_PERCENTAGE: f64 = 30.0;

    /// Time percentage threshold for second most consuming (G001b)
    pub const SECOND_CONSUMING_PERCENTAGE: f64 = 15.0;

    /// Default skew ratio threshold
    #[allow(dead_code)]
    pub const DEFAULT_SKEW_RATIO: f64 = 2.0;

    /// Default memory threshold (1GB)
    #[allow(dead_code)]
    pub const DEFAULT_MEMORY_THRESHOLD: u64 = 1024 * 1024 * 1024;

    /// Default hash table memory threshold (1GB)
    #[allow(dead_code)]
    pub const DEFAULT_HASH_TABLE_THRESHOLD: u64 = 1024 * 1024 * 1024;

    /// Minimum rows for skew detection
    #[allow(dead_code)]
    pub const MIN_ROWS_FOR_SKEW: f64 = 100_000.0;

    /// Minimum rows for filter effectiveness
    #[allow(dead_code)]
    pub const MIN_ROWS_FOR_FILTER: f64 = 100_000.0;

    /// Minimum rows for join explosion
    #[allow(dead_code)]
    pub const MIN_ROWS_FOR_JOIN: f64 = 10_000.0;

    /// Minimum IO time in nanoseconds for IO skew detection (500ms)
    pub const MIN_IO_TIME_NS: f64 = 500.0 * 1_000_000.0;

    /// Minimum execution time in nanoseconds for time skew detection (500ms)
    pub const MIN_EXEC_TIME_NS: u64 = 500 * 1_000_000;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_type_detection() {
        assert_eq!(QueryType::from_sql("SELECT * FROM t"), QueryType::Select);
        assert_eq!(QueryType::from_sql("INSERT INTO t1 SELECT * FROM t2"), QueryType::Insert);
        assert_eq!(QueryType::from_sql("EXPORT TABLE t TO 's3://'"), QueryType::Export);
        assert_eq!(QueryType::from_sql("ANALYZE TABLE t"), QueryType::Analyze);
        assert_eq!(QueryType::from_sql("CREATE TABLE t1 AS SELECT * FROM t2"), QueryType::Ctas);
    }

    #[test]
    fn test_time_thresholds() {
        assert_eq!(QueryType::Select.get_time_threshold_ms(), 10_000.0);
        assert_eq!(QueryType::Insert.get_time_threshold_ms(), 300_000.0);
        assert_eq!(QueryType::Load.get_time_threshold_ms(), 1_800_000.0);
    }

    #[test]
    fn test_skew_threshold_by_cluster_size() {
        let small_cluster = DynamicThresholds::new(
            ClusterInfo { backend_num: 4, ..Default::default() },
            QueryType::Select,
            QueryComplexity::Medium,
        );
        let large_cluster = DynamicThresholds::new(
            ClusterInfo { backend_num: 64, ..Default::default() },
            QueryType::Select,
            QueryComplexity::Medium,
        );

        assert!(
            large_cluster.get_skew_threshold() > small_cluster.get_skew_threshold(),
            "Large cluster should allow more skew"
        );
    }

    #[test]
    fn test_memory_threshold_clamping() {
        // Very small BE memory
        let small_be = DynamicThresholds::new(
            ClusterInfo {
                be_memory_limit: Some(4 * 1024 * 1024 * 1024), // 4GB
                ..Default::default()
            },
            QueryType::Select,
            QueryComplexity::Medium,
        );
        assert_eq!(
            small_be.get_operator_memory_threshold(),
            1024 * 1024 * 1024,
            "Should clamp to min 1GB"
        );

        // Very large BE memory
        let large_be = DynamicThresholds::new(
            ClusterInfo {
                be_memory_limit: Some(256 * 1024 * 1024 * 1024), // 256GB
                ..Default::default()
            },
            QueryType::Select,
            QueryComplexity::Medium,
        );
        assert_eq!(
            large_be.get_operator_memory_threshold(),
            10 * 1024 * 1024 * 1024,
            "Should clamp to max 10GB"
        );
    }

    #[test]
    fn test_small_file_threshold() {
        let thresholds = DynamicThresholds::default();

        assert_eq!(thresholds.get_small_file_threshold("S3"), 128 * 1024 * 1024);
        assert_eq!(thresholds.get_small_file_threshold("HDFS"), 64 * 1024 * 1024);
        assert_eq!(thresholds.get_small_file_threshold("LOCAL"), 32 * 1024 * 1024);
    }
}

// ============================================================================
// External Scan Type (Section 5 of review document)
// ============================================================================

/// External table scan type enumeration
/// Used for type-specific threshold calculation and suggestion generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalScanType {
    // Data lake formats
    /// Hive table scan
    Hive,
    /// Apache Iceberg table scan
    Iceberg,
    /// Apache Hudi table scan
    Hudi,
    /// Delta Lake table scan
    DeltaLake,
    /// Apache Paimon table scan
    Paimon,

    // File systems
    /// HDFS file scan
    Hdfs,
    /// Local file scan
    File,
    /// S3/OSS/COS/GCS object storage scan
    S3,

    // External databases
    /// JDBC external table scan
    Jdbc,
    /// MySQL external table scan
    Mysql,
    /// Elasticsearch external table scan
    Elasticsearch,

    // Generic connector
    /// Generic connector scan (fallback)
    Connector,
}

impl ExternalScanType {
    /// Detect external scan type from operator name
    pub fn from_operator_name(name: &str) -> Option<Self> {
        let upper = name.to_uppercase();

        // Data lake formats (check specific types first)
        if upper.contains("HIVE_SCAN") {
            return Some(Self::Hive);
        }
        if upper.contains("ICEBERG_SCAN") {
            return Some(Self::Iceberg);
        }
        if upper.contains("HUDI_SCAN") {
            return Some(Self::Hudi);
        }
        if upper.contains("DELTALAKE_SCAN") || upper.contains("DELTA_SCAN") {
            return Some(Self::DeltaLake);
        }
        if upper.contains("PAIMON_SCAN") {
            return Some(Self::Paimon);
        }

        // File systems
        if upper.contains("HDFS_SCAN") {
            return Some(Self::Hdfs);
        }
        if upper.contains("FILE_SCAN") {
            return Some(Self::File);
        }
        if upper.contains("S3_SCAN")
            || upper.contains("OSS_SCAN")
            || upper.contains("COS_SCAN")
            || upper.contains("GCS_SCAN")
        {
            return Some(Self::S3);
        }

        // External databases
        if upper.contains("JDBC_SCAN") {
            return Some(Self::Jdbc);
        }
        if upper.contains("MYSQL_SCAN") {
            return Some(Self::Mysql);
        }
        if upper.contains("ES_SCAN") || upper.contains("ELASTICSEARCH_SCAN") {
            return Some(Self::Elasticsearch);
        }

        // Generic connector (fallback for external tables)
        if upper.contains("CONNECTOR_SCAN") {
            return Some(Self::Connector);
        }

        None
    }

    /// Check if this scan type supports small file detection
    /// JDBC/MySQL/Elasticsearch don't have file-based storage
    pub fn supports_small_file_detection(&self) -> bool {
        matches!(
            self,
            Self::Hive
                | Self::Iceberg
                | Self::Hudi
                | Self::DeltaLake
                | Self::Paimon
                | Self::Hdfs
                | Self::File
                | Self::S3
                | Self::Connector
        )
    }

    /// Get storage type for threshold calculation
    pub fn storage_type(&self) -> &'static str {
        match self {
            Self::S3 => "S3",
            Self::Hdfs | Self::Hive => "HDFS",
            // Data lake formats typically use HDFS or S3, default to HDFS
            Self::Iceberg | Self::Hudi | Self::DeltaLake | Self::Paimon => "HDFS",
            Self::File => "LOCAL",
            Self::Connector => "HDFS", // Default for generic connector
            // Database types don't have file storage
            Self::Jdbc | Self::Mysql | Self::Elasticsearch => "UNKNOWN",
        }
    }

    /// Get the metric name for file count detection
    pub fn file_count_metric(&self) -> &'static str {
        match self {
            Self::Hdfs => "BlocksRead",
            _ => "ScanRanges",
        }
    }

    /// Get display name for this scan type
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Hive => "Hive",
            Self::Iceberg => "Iceberg",
            Self::Hudi => "Hudi",
            Self::DeltaLake => "Delta Lake",
            Self::Paimon => "Paimon",
            Self::Hdfs => "HDFS",
            Self::File => "File",
            Self::S3 => "S3/对象存储",
            Self::Jdbc => "JDBC",
            Self::Mysql => "MySQL",
            Self::Elasticsearch => "Elasticsearch",
            Self::Connector => "Connector",
        }
    }
}

/// Generate type-specific suggestions for small file issues
pub fn generate_small_file_suggestions(scan_type: &ExternalScanType, table: &str) -> Vec<String> {
    match scan_type {
        ExternalScanType::Hive => vec![
            format!("合并小文件: INSERT OVERWRITE {} SELECT * FROM {}", table, table),
            "调整 Hive 表的 mapreduce.input.fileinputformat.split.minsize".to_string(),
            "使用 ALTER TABLE CONCATENATE 合并小文件".to_string(),
        ],
        ExternalScanType::Iceberg => vec![
            format!("执行 Compaction: CALL rewrite_data_files(table => '{}')", table),
            "调整 write.target-file-size-bytes 参数（建议 128MB-256MB）".to_string(),
            "启用 Iceberg 自动 Compaction".to_string(),
        ],
        ExternalScanType::Hudi => vec![
            "执行 Hudi Compaction 合并小文件".to_string(),
            "调整 hoodie.parquet.small.file.limit 参数".to_string(),
            "检查 Hudi 表的 Compaction 策略配置".to_string(),
        ],
        ExternalScanType::DeltaLake => vec![
            format!("执行 OPTIMIZE {} ZORDER BY ...", table),
            "启用 Delta Lake Auto Compaction".to_string(),
            "调整 spark.databricks.delta.autoCompact.minNumFiles".to_string(),
        ],
        ExternalScanType::Paimon => vec![
            "执行 Paimon Compaction 合并小文件".to_string(),
            "调整 write.target-file-size 参数".to_string(),
            "检查 Paimon 表的 Compaction 配置".to_string(),
        ],
        ExternalScanType::Hdfs => vec![
            "使用 Hadoop Archive (HAR) 合并小文件".to_string(),
            "调整上游 ETL 输出文件大小（建议 128MB-256MB）".to_string(),
            "使用 Spark coalesce/repartition 合并输出文件".to_string(),
        ],
        ExternalScanType::S3 => vec![
            "使用 Spark/Flink 合并小文件".to_string(),
            "调整写入任务的并行度减少小文件产生".to_string(),
            "考虑使用更大的 Parquet row group size".to_string(),
            "对于 Iceberg/Hudi 表，执行 OPTIMIZE 或 Compaction".to_string(),
        ],
        ExternalScanType::File => {
            vec!["合并本地小文件".to_string(), "调整写入程序的输出文件大小".to_string()]
        },
        ExternalScanType::Connector => vec![
            "合并小文件以提升查询性能".to_string(),
            "考虑将热点数据导入 StarRocks 内表".to_string(),
            "调整上游数据写入的文件大小配置".to_string(),
        ],
        // Database types don't have small file issues
        ExternalScanType::Jdbc | ExternalScanType::Mysql | ExternalScanType::Elasticsearch => {
            vec![]
        },
    }
}

#[cfg(test)]
mod dynamic_thresholds_tests {
    use super::*;
    use crate::services::profile_analyzer::analyzer::baseline::BaselineStats;
    use crate::services::profile_analyzer::models::ClusterInfo;

    #[test]
    fn test_query_time_threshold_with_complexity() {
        let cluster_info = ClusterInfo { backend_num: 16, ..Default::default() };

        // Simple query - stricter threshold
        let simple_thresholds = DynamicThresholds::new(
            cluster_info.clone(),
            QueryType::Select,
            QueryComplexity::Simple,
        );
        let simple_threshold = simple_thresholds.get_query_time_threshold_ms();

        // Medium query - normal threshold
        let medium_thresholds = DynamicThresholds::new(
            cluster_info.clone(),
            QueryType::Select,
            QueryComplexity::Medium,
        );
        let medium_threshold = medium_thresholds.get_query_time_threshold_ms();

        // Complex query - relaxed threshold
        let complex_thresholds = DynamicThresholds::new(
            cluster_info.clone(),
            QueryType::Select,
            QueryComplexity::Complex,
        );
        let complex_threshold = complex_thresholds.get_query_time_threshold_ms();

        // VeryComplex query - most relaxed
        let very_complex_thresholds =
            DynamicThresholds::new(cluster_info, QueryType::Select, QueryComplexity::VeryComplex);
        let very_complex_threshold = very_complex_thresholds.get_query_time_threshold_ms();

        // Thresholds should increase with complexity
        assert!(simple_threshold < medium_threshold);
        assert!(medium_threshold < complex_threshold);
        assert!(complex_threshold < very_complex_threshold);
    }

    #[test]
    fn test_query_time_threshold_with_baseline() {
        let cluster_info = ClusterInfo { backend_num: 16, ..Default::default() };

        // Create baseline with specific stats
        let baseline = PerformanceBaseline {
            complexity: QueryComplexity::Medium,
            stats: BaselineStats {
                avg_ms: 5000.0,
                p50_ms: 4000.0,
                p95_ms: 8000.0,
                p99_ms: 12000.0,
                max_ms: 15000.0,
                std_dev_ms: 2000.0,
            },
            sample_size: 100,
            time_range_hours: 168,
        };

        let thresholds = DynamicThresholds::with_baseline(
            cluster_info,
            QueryType::Select,
            QueryComplexity::Medium,
            baseline,
        );

        let threshold = thresholds.get_query_time_threshold_ms();

        // Should be P95 + 2*std_dev = 8000 + 4000 = 12000
        // But minimum for Medium is 10000
        assert!(threshold >= 10000.0);
        assert!(threshold <= 15000.0);

        // With our baseline: 8000 + 2*2000 = 12000
        assert!((threshold - 12000.0).abs() < 1.0);
    }

    #[test]
    fn test_skew_threshold_by_cluster_size() {
        // Small cluster (8 BE) - stricter
        let small_cluster = ClusterInfo { backend_num: 8, ..Default::default() };
        let small_thresholds =
            DynamicThresholds::new(small_cluster, QueryType::Select, QueryComplexity::Medium);
        let small_skew = small_thresholds.get_skew_threshold();

        // Medium cluster (16 BE)
        let medium_cluster = ClusterInfo { backend_num: 16, ..Default::default() };
        let medium_thresholds =
            DynamicThresholds::new(medium_cluster, QueryType::Select, QueryComplexity::Medium);
        let medium_skew = medium_thresholds.get_skew_threshold();

        // Large cluster (64 BE) - more tolerant
        let large_cluster = ClusterInfo { backend_num: 64, ..Default::default() };
        let large_thresholds =
            DynamicThresholds::new(large_cluster, QueryType::Select, QueryComplexity::Medium);
        let large_skew = large_thresholds.get_skew_threshold();

        // Larger clusters should have higher skew tolerance
        assert!(small_skew < medium_skew);
        assert!(medium_skew < large_skew);

        // Verify ranges
        assert!((2.0..=2.5).contains(&small_skew));
        assert!(large_skew >= 3.5);
    }

    #[test]
    fn test_skew_threshold_with_baseline() {
        let cluster_info = ClusterInfo { backend_num: 16, ..Default::default() };

        // Create baseline with high variance (P99/P50 = 3.0)
        let baseline = PerformanceBaseline {
            complexity: QueryComplexity::Medium,
            stats: BaselineStats {
                avg_ms: 5000.0,
                p50_ms: 4000.0,
                p95_ms: 8000.0,
                p99_ms: 12000.0, // P99/P50 = 3.0
                max_ms: 15000.0,
                std_dev_ms: 2000.0,
            },
            sample_size: 100,
            time_range_hours: 168,
        };

        let thresholds = DynamicThresholds::with_baseline(
            cluster_info,
            QueryType::Select,
            QueryComplexity::Medium,
            baseline,
        );

        let skew_threshold = thresholds.get_skew_threshold();

        // Base for 16 BE: 3.0
        // Historical adjustment: (3.0 - 2.0) * 0.2 = 0.2
        // Final: 3.0 + 0.2 = 3.2
        assert!(skew_threshold > 3.0);
        assert!(skew_threshold < 4.0);
    }

    #[test]
    fn test_detect_complexity() {
        // Simple
        let sql1 = "SELECT * FROM users WHERE id = 1";
        assert_eq!(DynamicThresholds::detect_complexity(sql1), QueryComplexity::Simple);

        // Medium (with JOIN)
        let sql2 = "SELECT * FROM users u JOIN orders o ON u.id = o.user_id";
        assert_eq!(DynamicThresholds::detect_complexity(sql2), QueryComplexity::Medium);
    }

    #[test]
    fn test_min_threshold_by_complexity() {
        let cluster_info = ClusterInfo::default();

        let simple = DynamicThresholds::new(
            cluster_info.clone(),
            QueryType::Select,
            QueryComplexity::Simple,
        );
        let medium = DynamicThresholds::new(
            cluster_info.clone(),
            QueryType::Select,
            QueryComplexity::Medium,
        );
        let complex = DynamicThresholds::new(
            cluster_info.clone(),
            QueryType::Select,
            QueryComplexity::Complex,
        );
        let very_complex =
            DynamicThresholds::new(cluster_info, QueryType::Select, QueryComplexity::VeryComplex);

        // Minimum thresholds
        assert!(simple.get_query_time_threshold_ms() >= 5_000.0);
        assert!(medium.get_query_time_threshold_ms() >= 10_000.0);
        assert!(complex.get_query_time_threshold_ms() >= 30_000.0);
        assert!(very_complex.get_query_time_threshold_ms() >= 60_000.0);
    }
}

#[cfg(test)]
mod external_scan_type_tests {
    use super::*;

    #[test]
    fn test_external_scan_type_detection() {
        // Data lake formats
        assert_eq!(ExternalScanType::from_operator_name("HIVE_SCAN"), Some(ExternalScanType::Hive));
        assert_eq!(
            ExternalScanType::from_operator_name("ICEBERG_SCAN"),
            Some(ExternalScanType::Iceberg)
        );
        assert_eq!(ExternalScanType::from_operator_name("HUDI_SCAN"), Some(ExternalScanType::Hudi));
        assert_eq!(
            ExternalScanType::from_operator_name("DELTALAKE_SCAN"),
            Some(ExternalScanType::DeltaLake)
        );
        assert_eq!(
            ExternalScanType::from_operator_name("PAIMON_SCAN"),
            Some(ExternalScanType::Paimon)
        );

        // File systems
        assert_eq!(ExternalScanType::from_operator_name("HDFS_SCAN"), Some(ExternalScanType::Hdfs));
        assert_eq!(ExternalScanType::from_operator_name("FILE_SCAN"), Some(ExternalScanType::File));
        assert_eq!(ExternalScanType::from_operator_name("S3_SCAN"), Some(ExternalScanType::S3));

        // External databases
        assert_eq!(ExternalScanType::from_operator_name("JDBC_SCAN"), Some(ExternalScanType::Jdbc));
        assert_eq!(
            ExternalScanType::from_operator_name("MYSQL_SCAN"),
            Some(ExternalScanType::Mysql)
        );
        assert_eq!(
            ExternalScanType::from_operator_name("ES_SCAN"),
            Some(ExternalScanType::Elasticsearch)
        );

        // Generic connector
        assert_eq!(
            ExternalScanType::from_operator_name("CONNECTOR_SCAN"),
            Some(ExternalScanType::Connector)
        );

        // Non-external scans
        assert_eq!(ExternalScanType::from_operator_name("OLAP_SCAN"), None);
    }

    #[test]
    fn test_small_file_detection_support() {
        // File-based types support small file detection
        assert!(ExternalScanType::Hive.supports_small_file_detection());
        assert!(ExternalScanType::Iceberg.supports_small_file_detection());
        assert!(ExternalScanType::Hdfs.supports_small_file_detection());
        assert!(ExternalScanType::S3.supports_small_file_detection());

        // Database types don't support small file detection
        assert!(!ExternalScanType::Jdbc.supports_small_file_detection());
        assert!(!ExternalScanType::Mysql.supports_small_file_detection());
        assert!(!ExternalScanType::Elasticsearch.supports_small_file_detection());
    }

    #[test]
    fn test_storage_type() {
        assert_eq!(ExternalScanType::S3.storage_type(), "S3");
        assert_eq!(ExternalScanType::Hdfs.storage_type(), "HDFS");
        assert_eq!(ExternalScanType::Hive.storage_type(), "HDFS");
        assert_eq!(ExternalScanType::File.storage_type(), "LOCAL");
        assert_eq!(ExternalScanType::Iceberg.storage_type(), "HDFS");
    }

    #[test]
    fn test_file_count_metric() {
        assert_eq!(ExternalScanType::Hdfs.file_count_metric(), "BlocksRead");
        assert_eq!(ExternalScanType::Hive.file_count_metric(), "ScanRanges");
        assert_eq!(ExternalScanType::S3.file_count_metric(), "ScanRanges");
    }

    #[test]
    fn test_generate_suggestions() {
        let hive_suggestions =
            generate_small_file_suggestions(&ExternalScanType::Hive, "test_table");
        assert!(!hive_suggestions.is_empty());
        assert!(hive_suggestions[0].contains("test_table"));

        let iceberg_suggestions =
            generate_small_file_suggestions(&ExternalScanType::Iceberg, "iceberg_table");
        assert!(
            iceberg_suggestions
                .iter()
                .any(|s| s.contains("rewrite_data_files"))
        );

        // Database types return empty suggestions
        let jdbc_suggestions =
            generate_small_file_suggestions(&ExternalScanType::Jdbc, "jdbc_table");
        assert!(jdbc_suggestions.is_empty());
    }
}
