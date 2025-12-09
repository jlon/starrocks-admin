//! StarRocks Profile Analyzer
//!
//! A comprehensive module for parsing, analyzing, and visualizing StarRocks query profiles.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    ProfileAnalyzer                          │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │                   analyze_profile()                  │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                           │                                 │
//! │           ┌───────────────┼───────────────┐                │
//! │           ▼               ▼               ▼                │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
//! │  │   Parser    │  │  Analyzer   │  │   Models    │        │
//! │  │  ┌───────┐  │  │  ┌───────┐  │  │             │        │
//! │  │  │Composer│  │  │  │Hotspot│  │  │  Profile    │        │
//! │  │  └───────┘  │  │  │Detector│  │  │  Summary    │        │
//! │  │  ┌───────┐  │  │  └───────┘  │  │  ExecTree   │        │
//! │  │  │Section│  │  │  ┌───────┐  │  │  Fragment   │        │
//! │  │  │Parser │  │  │  │Suggest│  │  │  ...        │        │
//! │  │  └───────┘  │  │  │Engine │  │  │             │        │
//! │  │  ┌───────┐  │  │  └───────┘  │  │             │        │
//! │  │  │Topology│ │  │             │  │             │        │
//! │  │  │Parser │  │  │             │  │             │        │
//! │  │  └───────┘  │  │             │  │             │        │
//! │  └─────────────┘  └─────────────┘  └─────────────┘        │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use starrocks_admin_backend::services::profile_analyzer::analyze_profile;
//!
//! let profile_text = "..."; // Raw profile text from StarRocks
//! let result = analyze_profile(profile_text)?;
//!
//! // Access parsed data
//! println!("Query ID: {}", result.summary.as_ref().unwrap().query_id);
//! println!("Performance Score: {}", result.performance_score);
//! ```

pub mod analyzer;
pub mod models;
pub mod parser;

#[cfg(test)]
mod tests;

pub use analyzer::RuleEngine;
pub use models::*;
pub use parser::ProfileComposer;

use std::collections::HashMap;

/// Cluster session variables fetched from the live cluster
/// These are the actual current values, not just non-default ones
pub type ClusterVariables = HashMap<String, String>;

/// Analysis context containing optional cluster variables
#[derive(Default)]
pub struct AnalysisContext {
    /// Live cluster variables (actual current values)
    pub cluster_variables: Option<ClusterVariables>,
    /// Cluster ID for baseline lookup
    pub cluster_id: Option<i64>,
}

/// Analyze a profile text and return complete analysis results
///
/// This is the main entry point for profile analysis. It:
/// 1. Parses the profile text into structured data
/// 2. Builds the execution tree for DAG visualization
/// 3. Detects performance hotspots
/// 4. Generates optimization suggestions
/// 5. Calculates a performance score
///
/// # Arguments
///
/// * `profile_text` - Raw profile text from StarRocks (from `get_query_profile()` or `SHOW PROFILE`)
///
/// # Returns
///
/// * `Ok(ProfileAnalysisResponse)` - Complete analysis results
/// * `Err(String)` - Error message if parsing fails
///
/// # Example
///
/// ```ignore
/// let result = analyze_profile(profile_text)?;
///
/// // Check for hotspots
/// for hotspot in &result.hotspots {
///     println!("{}: {}", hotspot.node_path, hotspot.description);
/// }
///
/// // Access execution tree for visualization
/// if let Some(tree) = &result.execution_tree {
///     for node in &tree.nodes {
///         println!("{}: {:.2}%", node.operator_name, node.time_percentage.unwrap_or(0.0));
///     }
/// }
/// ```
/// Simple analysis without cluster context (for backward compatibility and tests)
#[allow(dead_code)]
pub fn analyze_profile(profile_text: &str) -> Result<ProfileAnalysisResponse, String> {
    analyze_profile_with_context(profile_text, &AnalysisContext::default())
}

/// Analyze a profile with additional context (e.g., live cluster variables)
pub fn analyze_profile_with_context(
    profile_text: &str,
    context: &AnalysisContext,
) -> Result<ProfileAnalysisResponse, String> {
    let mut composer = ProfileComposer::new();
    let profile = composer
        .parse(profile_text)
        .map_err(|e| format!("解析Profile失败: {:?}", e))?;

    let execution_tree = profile.execution_tree.clone();
    let mut summary = profile.summary.clone();

    // Calculate DataCache hit rate directly from profile text
    // This handles nested metrics like DataCache: -> DataCacheReadDiskBytes
    let (total_local, total_remote) = extract_datacache_from_text(profile_text);
    tracing::info!(
        "DataCache metrics - Local: {} bytes ({:.2} GB), Remote: {} bytes ({:.2} GB)",
        total_local,
        total_local as f64 / 1024.0 / 1024.0 / 1024.0,
        total_remote,
        total_remote as f64 / 1024.0 / 1024.0 / 1024.0
    );
    if total_local > 0 || total_remote > 0 {
        let total = total_local + total_remote;
        let hit_rate = total_local as f64 / total as f64;
        tracing::info!("DataCache hit rate: {:.2}%", hit_rate * 100.0);
        summary.datacache_hit_rate = Some(hit_rate);
        summary.datacache_bytes_local = Some(total_local);
        summary.datacache_bytes_remote = Some(total_remote);
        summary.datacache_bytes_local_display = Some(format_bytes_display(total_local));
        summary.datacache_bytes_remote_display = Some(format_bytes_display(total_remote));
    }

    // Aggregate IO metrics from scan nodes
    if let Some(ref tree) = execution_tree {
        let io_stats = aggregate_io_statistics(&tree.nodes);
        summary.total_raw_rows_read = io_stats.raw_rows_read;
        summary.total_bytes_read = io_stats.bytes_read;
        if let Some(bytes) = io_stats.bytes_read {
            summary.total_bytes_read_display = Some(format_bytes_display(bytes));
        }
        summary.pages_count_memory = io_stats.pages_count_memory;
        summary.pages_count_local_disk = io_stats.pages_count_local_disk;
        summary.pages_count_remote = io_stats.pages_count_remote;
        summary.result_rows = io_stats.result_rows;
        summary.result_bytes = io_stats.result_bytes;
        if let Some(bytes) = io_stats.result_bytes {
            summary.result_bytes_display = Some(format_bytes_display(bytes));
        }
        // IO time metrics
        if let Some(ms) = io_stats.io_seek_time_ms {
            summary.io_seek_time_ms = Some(ms);
            summary.io_seek_time = Some(format_duration_ms(ms));
        }
        if let Some(ms) = io_stats.local_disk_read_io_time_ms {
            summary.local_disk_read_io_time_ms = Some(ms);
            summary.local_disk_read_io_time = Some(format_duration_ms(ms));
        }
        if let Some(ms) = io_stats.remote_read_io_time_ms {
            summary.remote_read_io_time_ms = Some(ms);
            summary.remote_read_io_time = Some(format_duration_ms(ms));
        }
    }

    let summary = Some(summary);
    let mut execution_tree = execution_tree;

    // Run RuleEngine for diagnostics with optional cluster variables and baseline
    let rule_engine = RuleEngine::new();
    let rule_diagnostics = rule_engine.analyze_with_baseline(
        &profile,
        context.cluster_variables.as_ref(),
        context.cluster_id,
    );

    // Convert rule diagnostics to API response format
    let diagnostics: Vec<DiagnosticResult> = rule_diagnostics
        .iter()
        .map(|d| DiagnosticResult {
            rule_id: d.rule_id.clone(),
            rule_name: d.rule_name.clone(),
            severity: format!("{:?}", d.severity),
            node_path: d.node_path.clone(),
            plan_node_id: d.plan_node_id,
            message: d.message.clone(),
            reason: d.reason.clone(),
            suggestions: d.suggestions.clone(),
            parameter_suggestions: d
                .parameter_suggestions
                .iter()
                .map(|p| ParameterTuningSuggestion {
                    name: p.name.clone(),
                    param_type: format!("{:?}", p.param_type),
                    current: p.current.clone(),
                    recommended: p.recommended.clone(),
                    command: p.command.clone(),
                    description: p.description.clone(),
                    impact: p.impact.clone(),
                })
                .collect(),
            // Pass through threshold metadata for LLM traceability
            threshold_metadata: d
                .threshold_metadata
                .as_ref()
                .map(|tm| ThresholdMetadataResult {
                    threshold_value: tm.threshold_value,
                    threshold_source: tm.threshold_source.clone(),
                    baseline_p95_ms: tm.baseline_p95_ms,
                    baseline_sample_count: tm.baseline_sample_count,
                }),
        })
        .collect();

    // Build node_diagnostics mapping (plan_node_id -> diagnostics)
    let mut node_diagnostics: HashMap<i32, Vec<DiagnosticResult>> = HashMap::new();
    for diag in &diagnostics {
        if let Some(plan_node_id) = diag.plan_node_id {
            node_diagnostics
                .entry(plan_node_id)
                .or_default()
                .push(diag.clone());
        }
    }

    // Update execution tree nodes with diagnostic info
    if let Some(ref mut tree) = execution_tree {
        for node in &mut tree.nodes {
            if let Some(plan_node_id) = node.plan_node_id
                && let Some(node_diags) = node_diagnostics.get(&plan_node_id)
            {
                node.has_diagnostic = true;
                node.diagnostic_ids = node_diags.iter().map(|d| d.rule_id.clone()).collect();
            }
        }
    }

    // Aggregate diagnostics by rule_id for overview display
    let aggregated_diagnostics = aggregate_diagnostics(&diagnostics);

    // Generate hotspots from diagnostics for backward compatibility
    let hotspots: Vec<HotSpot> = rule_diagnostics.iter().map(|d| d.to_hotspot()).collect();

    // Generate conclusion, suggestions and performance score using RuleEngine
    let conclusion = RuleEngine::generate_conclusion(&rule_diagnostics, &profile);
    let all_suggestions = RuleEngine::generate_suggestions(&rule_diagnostics);
    let performance_score = RuleEngine::calculate_performance_score(&rule_diagnostics, &profile);

    // Perform root cause analysis (v5.0 - rule-based, without LLM)
    let root_cause_analysis = if !rule_diagnostics.is_empty() {
        Some(analyzer::RootCauseAnalyzer::analyze(&rule_diagnostics))
    } else {
        None
    };

    Ok(ProfileAnalysisResponse {
        hotspots,
        conclusion,
        suggestions: all_suggestions,
        performance_score,
        execution_tree,
        summary,
        diagnostics,
        aggregated_diagnostics,
        node_diagnostics,
        profile_content: Some(profile_text.to_string()),
        fragments: profile.fragments.clone(),
        root_cause_analysis,
        llm_analysis: None, // Filled by handler if LLM is enabled
    })
}

/// Aggregate diagnostics by rule_id for overview display
/// Groups multiple diagnostics of the same rule together
fn aggregate_diagnostics(diagnostics: &[DiagnosticResult]) -> Vec<AggregatedDiagnostic> {
    use std::collections::HashMap;

    // Group by rule_id
    let mut groups: HashMap<String, Vec<&DiagnosticResult>> = HashMap::new();
    for diag in diagnostics {
        groups.entry(diag.rule_id.clone()).or_default().push(diag);
    }

    // Convert groups to aggregated diagnostics
    let mut result: Vec<AggregatedDiagnostic> = groups
        .into_iter()
        .map(|(rule_id, diags)| {
            let first = diags.first().unwrap();
            let affected_nodes: Vec<String> = diags.iter().map(|d| d.node_path.clone()).collect();
            let node_count = affected_nodes.len();

            // Smart suggestion merging: dedupe by pattern, not exact match
            let suggestions = merge_suggestions(&diags, node_count);

            // Merge parameter suggestions (take first non-empty)
            let parameter_suggestions = diags
                .iter()
                .find(|d| !d.parameter_suggestions.is_empty())
                .map(|d| d.parameter_suggestions.clone())
                .unwrap_or_default();

            // Determine highest severity
            let severity = diags
                .iter()
                .map(|d| &d.severity)
                .max_by(|a, b| severity_order(a).cmp(&severity_order(b)))
                .unwrap_or(&first.severity)
                .clone();

            // Generate aggregated message
            let message = if node_count > 1 {
                format!("{} 个节点存在此问题", node_count)
            } else {
                first.message.clone()
            };

            AggregatedDiagnostic {
                rule_id,
                rule_name: first.rule_name.clone(),
                severity,
                message,
                reason: first.reason.clone(),
                affected_nodes,
                node_count,
                suggestions,
                parameter_suggestions,
            }
        })
        .collect();

    // Sort by severity (Error > Warning > Info) then by node_count
    result.sort_by(|a, b| {
        let severity_cmp = severity_order(&b.severity).cmp(&severity_order(&a.severity));
        if severity_cmp != std::cmp::Ordering::Equal {
            severity_cmp
        } else {
            b.node_count.cmp(&a.node_count)
        }
    });

    result
}

/// Merge suggestions intelligently to avoid repetition
/// When multiple nodes have similar suggestions (differing only by table name), consolidate them
fn merge_suggestions(diags: &[&DiagnosticResult], node_count: usize) -> Vec<String> {
    if node_count == 1 {
        return diags
            .first()
            .map(|d| d.suggestions.clone())
            .unwrap_or_default();
    }

    // Extract table names from reason field (format: "外表「table_name」的 ORC...")
    let tables: Vec<String> = diags
        .iter()
        .filter_map(|d| extract_table_from_reason(&d.reason))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Collect first suggestion to check pattern
    let first_sug = diags
        .first()
        .and_then(|d| d.suggestions.first())
        .map(|s| s.as_str())
        .unwrap_or("");

    // Check if this looks like file fragmentation suggestion
    if first_sug.contains("外表小文件合并方案") || first_sug.contains("Compaction 合并碎片")
    {
        let tables_str = if tables.is_empty() {
            format!("{} 个表", node_count)
        } else if tables.len() <= 3 {
            tables.join(", ")
        } else {
            format!("{} 等 {} 个表", tables.first().unwrap_or(&"unknown".to_string()), tables.len())
        };

        let generic = if first_sug.contains("外表小文件合并方案") {
            format!(
                "外表小文件合并 (涉及: {}): ①ALTER TABLE <table> PARTITION(...) CONCATENATE; \
                 ②INSERT OVERWRITE TABLE <table> SELECT * FROM <table>; \
                 ③Spark: df.repartition(N).saveAsTable('<table>'); \
                 ④SET connector_io_tasks_per_scan_operator=64",
                tables_str
            )
        } else {
            format!("执行 Compaction: ALTER TABLE <{}> COMPACT", tables_str)
        };
        return vec![generic];
    }

    // Default: dedupe by exact match but limit to 3
    let mut seen = std::collections::HashSet::new();
    diags
        .iter()
        .flat_map(|d| d.suggestions.iter())
        .filter(|s| seen.insert(s.as_str()))
        .take(3)
        .cloned()
        .collect()
}

/// Extract table name from reason field like "外表「table_name」的 ORC..."
fn extract_table_from_reason(reason: &str) -> Option<String> {
    let start = reason.find('「')?;
    let end = reason.find('」')?;
    if end > start {
        // Skip the 「 character (3 bytes in UTF-8)
        Some(reason[start + 3..end].to_string())
    } else {
        None
    }
}

/// Get severity order for sorting (higher = more severe)
fn severity_order(severity: &str) -> u8 {
    match severity {
        "Error" => 3,
        "Warning" => 2,
        "Info" => 1,
        _ => 0,
    }
}

/// Extract DataCache metrics directly from profile text
///
/// Supports three storage architectures:
/// 1. **存算一体 (Shared-Nothing)**: OLAP_SCAN - no cache metrics, data is local
/// 2. **存算分离 (Shared-Data/Lake)**: CONNECTOR_SCAN with LakeConnector
///    - CompressedBytesReadLocalDisk: local cache hit
///    - CompressedBytesReadRemote: remote read (cache miss)
/// 3. **外部表 (External Tables)**: HDFS_SCAN / CONNECTOR_SCAN with HiveConnector
///    - DataCacheReadDiskBytes: local disk cache hit
///    - DataCacheReadMemBytes: memory cache hit  
///    - FSIOBytesRead: cache miss, read from remote HDFS (key metric!)
///    - DataCacheSkipReadBytes: actively bypassed cache
///
/// **Important**: For external tables, FSIOBytesRead represents actual cache misses,
/// while DataCacheSkipReadBytes only counts actively skipped reads.
///
/// Returns (total_cache_hit_bytes, total_remote_read_bytes)
fn extract_datacache_from_text(profile_text: &str) -> (u64, u64) {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // ========== 存算分离 (Lake/Shared-Data) metrics ==========
    // These are under IOStatistics: section in CONNECTOR_SCAN
    static COMPRESSED_LOCAL_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"- CompressedBytesReadLocalDisk:\s*([0-9.]+)\s*(B|KB|MB|GB|TB)").unwrap()
    });
    static COMPRESSED_REMOTE_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"- CompressedBytesReadRemote:\s*([0-9.]+)\s*(B|KB|MB|GB|TB)").unwrap()
    });

    // ========== 外部表 (Hive/Iceberg) DataCache metrics ==========
    // These are under DataCache: section in CONNECTOR_SCAN/HDFS_SCAN
    static DATACACHE_DISK_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"- DataCacheReadDiskBytes:\s*([0-9.]+)\s*(B|KB|MB|GB|TB)").unwrap()
    });
    static DATACACHE_MEM_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"- DataCacheReadMemBytes:\s*([0-9.]+)\s*(B|KB|MB|GB|TB)").unwrap()
    });
    static DATACACHE_SKIP_READ_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"- DataCacheSkipReadBytes:\s*([0-9.]+)\s*(B|KB|MB|GB|TB)").unwrap()
    });

    // ========== 外部表 InputStream metrics (cache miss indicator) ==========
    // FSIOBytesRead = actual bytes read from remote HDFS when cache misses
    static FSIO_BYTES_READ_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"- FSIOBytesRead:\s*([0-9.]+)\s*(B|KB|MB|GB|TB)").unwrap());

    let mut total_local: u64 = 0;
    let mut total_remote: u64 = 0;
    let mut fsio_bytes: u64 = 0;
    let mut datacache_skip_bytes: u64 = 0;
    let mut has_datacache_metrics = false;

    // Helper function to parse bytes value with unit
    let parse_bytes = |value: &str, unit: &str| -> u64 {
        let v: f64 = value.parse().unwrap_or(0.0);
        let multiplier: u64 = match unit {
            "TB" => 1024 * 1024 * 1024 * 1024,
            "GB" => 1024 * 1024 * 1024,
            "MB" => 1024 * 1024,
            "KB" => 1024,
            _ => 1,
        };
        (v * multiplier as f64) as u64
    };

    // Parse each line, skip __MAX_OF_ and __MIN_OF_ (only count aggregated values)
    for line in profile_text.lines() {
        let trimmed = line.trim();

        // Skip min/max variants, only count the main aggregated value
        if trimmed.contains("__MAX_OF_") || trimmed.contains("__MIN_OF_") {
            continue;
        }

        // ===== 存算分离 (Lake) =====
        // CompressedBytesReadLocalDisk - data read from local cache
        if let Some(caps) = COMPRESSED_LOCAL_REGEX.captures(trimmed) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("B");
            total_local += parse_bytes(value, unit);
        }

        // CompressedBytesReadRemote - data read from remote storage
        if let Some(caps) = COMPRESSED_REMOTE_REGEX.captures(trimmed) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("B");
            total_remote += parse_bytes(value, unit);
        }

        // ===== 外部表 (Hive/Iceberg) =====
        // DataCacheReadDiskBytes - data read from local disk cache
        if let Some(caps) = DATACACHE_DISK_REGEX.captures(trimmed) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("B");
            total_local += parse_bytes(value, unit);
            has_datacache_metrics = true;
        }

        // DataCacheReadMemBytes - data read from memory cache
        if let Some(caps) = DATACACHE_MEM_REGEX.captures(trimmed) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("B");
            total_local += parse_bytes(value, unit);
            has_datacache_metrics = true;
        }

        // DataCacheSkipReadBytes - data that actively bypassed cache
        if let Some(caps) = DATACACHE_SKIP_READ_REGEX.captures(trimmed) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("B");
            datacache_skip_bytes += parse_bytes(value, unit);
        }

        // FSIOBytesRead - actual bytes read from remote HDFS (cache miss)
        if let Some(caps) = FSIO_BYTES_READ_REGEX.captures(trimmed) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let unit = caps.get(2).map(|m| m.as_str()).unwrap_or("B");
            fsio_bytes += parse_bytes(value, unit);
        }
    }

    // For external tables with DataCache:
    // - DataCacheReadDiskBytes/DataCacheReadMemBytes = cache hit (local)
    // - FSIOBytesRead = actual bytes read from remote HDFS/S3 (cache miss!)
    // - DataCacheSkipReadBytes = policy-based skipped reads (rare)
    //
    // The correct formula for cache hit rate:
    //   hit_rate = DataCacheReadBytes / (DataCacheReadBytes + FSIOBytesRead)
    //
    // FSIOBytesRead is the TRUE cache miss indicator, NOT DataCacheSkipReadBytes!
    if has_datacache_metrics {
        // FSIOBytesRead represents actual remote reads when cache misses
        // This is the primary indicator of cache miss
        total_remote += fsio_bytes;

        // DataCacheSkipReadBytes only adds if explicitly skipped (policy-based)
        // Don't add it again if already counted in FSIOBytesRead
        if datacache_skip_bytes > 0 && fsio_bytes == 0 {
            total_remote += datacache_skip_bytes;
        }
    } else {
        // For Lake storage without DataCache metrics, use skip bytes
        total_remote += datacache_skip_bytes;
    }

    (total_local, total_remote)
}

/// Calculate total DataCache bytes from execution tree nodes (legacy, kept for reference)
/// Returns (total_local_bytes, total_remote_bytes)
/// Supports both OLAP_SCAN (disaggregated storage) and HDFS_SCAN (external tables)
#[allow(dead_code)]
fn calculate_datacache_totals(nodes: &[ExecutionTreeNode]) -> (u64, u64) {
    let mut total_local: u64 = 0;
    let mut total_remote: u64 = 0;

    for node in nodes {
        // Only check SCAN nodes
        if !node.operator_name.to_uppercase().contains("SCAN") {
            continue;
        }

        // Try OLAP_SCAN metrics first (disaggregated storage-compute)
        if let Some(local_str) = node.unique_metrics.get("CompressedBytesReadLocalDisk")
            && let Ok(bytes) = parser::core::ValueParser::parse_bytes(local_str)
        {
            total_local += bytes;
        }
        if let Some(remote_str) = node.unique_metrics.get("CompressedBytesReadRemote")
            && let Ok(bytes) = parser::core::ValueParser::parse_bytes(remote_str)
        {
            total_remote += bytes;
        }

        // Try HDFS_SCAN / Hive Connector metrics (external tables with DataCache)
        // DataCacheReadDiskBytes = bytes read from local disk cache
        // DataCacheReadMemBytes = bytes read from memory cache
        // Total cache hit = DataCacheReadDiskBytes + DataCacheReadMemBytes
        let mut hdfs_cache_hit: u64 = 0;
        if let Some(disk_str) = node.unique_metrics.get("DataCacheReadDiskBytes")
            && let Ok(bytes) = parser::core::ValueParser::parse_bytes(disk_str)
        {
            hdfs_cache_hit += bytes;
        }
        if let Some(mem_str) = node.unique_metrics.get("DataCacheReadMemBytes")
            && let Ok(bytes) = parser::core::ValueParser::parse_bytes(mem_str)
        {
            hdfs_cache_hit += bytes;
        }

        // For HDFS_SCAN, we need to calculate remote bytes from total - cache
        // BytesRead or RawBytesRead = total bytes read
        if hdfs_cache_hit > 0 {
            total_local += hdfs_cache_hit;

            // Try to get total bytes read to calculate remote
            let mut total_read: u64 = 0;
            if let Some(total_str) = node.unique_metrics.get("BytesRead")
                && let Ok(bytes) = parser::core::ValueParser::parse_bytes(total_str)
            {
                total_read = bytes;
            }
            if total_read == 0
                && let Some(total_str) = node.unique_metrics.get("RawBytesRead")
                && let Ok(bytes) = parser::core::ValueParser::parse_bytes(total_str)
            {
                total_read = bytes;
            }

            // Remote = Total - CacheHit (if total > cache hit)
            if total_read > hdfs_cache_hit {
                total_remote += total_read - hdfs_cache_hit;
            }
        }
    }

    (total_local, total_remote)
}

/// Format bytes to human-readable display string
fn format_bytes_display(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format milliseconds to human-readable duration string
fn format_duration_ms(ms: f64) -> String {
    if ms < 1.0 {
        format!("{:.2}us", ms * 1000.0)
    } else if ms < 1000.0 {
        format!("{:.2}ms", ms)
    } else if ms < 60000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else {
        let minutes = (ms / 60000.0).floor();
        let seconds = (ms % 60000.0) / 1000.0;
        format!("{:.0}m{:.2}s", minutes, seconds)
    }
}

/// Aggregated IO statistics from scan nodes
#[derive(Default)]
struct IoStatistics {
    raw_rows_read: Option<u64>,
    bytes_read: Option<u64>,
    pages_count_memory: Option<u64>,
    pages_count_local_disk: Option<u64>,
    pages_count_remote: Option<u64>,
    result_rows: Option<u64>,
    result_bytes: Option<u64>,
    io_seek_time_ms: Option<f64>,
    local_disk_read_io_time_ms: Option<f64>,
    remote_read_io_time_ms: Option<f64>,
}

/// Aggregate IO statistics from all scan nodes in the execution tree
fn aggregate_io_statistics(nodes: &[ExecutionTreeNode]) -> IoStatistics {
    let mut stats = IoStatistics::default();
    let mut total_raw_rows: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_pages_memory: u64 = 0;
    let mut total_pages_local: u64 = 0;
    let mut total_pages_remote: u64 = 0;
    let mut total_result_rows: u64 = 0;
    let mut total_result_bytes: u64 = 0;
    let mut total_io_seek_ms: f64 = 0.0;
    let mut total_local_io_ms: f64 = 0.0;
    let mut total_remote_io_ms: f64 = 0.0;
    let mut has_any_scan = false;
    let mut has_any_sink = false;

    for node in nodes {
        let name = node.operator_name.to_uppercase();

        // Aggregate from SCAN nodes
        if name.contains("SCAN") {
            has_any_scan = true;

            // RawRowsRead
            if let Some(val) = node.unique_metrics.get("RawRowsRead")
                && let Ok(rows) = val.parse::<u64>()
            {
                total_raw_rows += rows;
            }

            // BytesRead
            if let Some(val) = node.unique_metrics.get("BytesRead")
                && let Ok(bytes) = parser::core::ValueParser::parse_bytes(val)
            {
                total_bytes += bytes;
            }

            // PagesCountMemory
            if let Some(val) = node.unique_metrics.get("PagesCountMemory")
                && let Ok(pages) = val.parse::<u64>()
            {
                total_pages_memory += pages;
            }

            // PagesCountLocalDisk
            if let Some(val) = node.unique_metrics.get("PagesCountLocalDisk")
                && let Ok(pages) = val.parse::<u64>()
            {
                total_pages_local += pages;
            }

            // PagesCountRemote
            if let Some(val) = node.unique_metrics.get("PagesCountRemote")
                && let Ok(pages) = val.parse::<u64>()
            {
                total_pages_remote += pages;
            }

            // IO time metrics (for disaggregated storage)
            // IoSeekTime
            if let Some(val) = node.unique_metrics.get("IoSeekTime")
                && let Ok(ms) = parser::core::ValueParser::parse_time_to_ms(val)
            {
                total_io_seek_ms += ms;
            }

            // IOTimeLocalDisk
            if let Some(val) = node.unique_metrics.get("IOTimeLocalDisk")
                && let Ok(ms) = parser::core::ValueParser::parse_time_to_ms(val)
            {
                total_local_io_ms += ms;
            }

            // IOTimeRemote
            if let Some(val) = node.unique_metrics.get("IOTimeRemote")
                && let Ok(ms) = parser::core::ValueParser::parse_time_to_ms(val)
            {
                total_remote_io_ms += ms;
            }
        }

        // Aggregate from SINK nodes for result metrics
        if name.contains("SINK") {
            has_any_sink = true;

            // ResultRows / RowsReturned
            if let Some(val) = node
                .unique_metrics
                .get("RowsReturned")
                .or_else(|| node.unique_metrics.get("NumSentRows"))
                && let Ok(rows) = val.parse::<u64>()
            {
                total_result_rows += rows;
            }

            // ResultBytes
            if let Some(val) = node.unique_metrics.get("BytesSent")
                && let Ok(bytes) = parser::core::ValueParser::parse_bytes(val)
            {
                total_result_bytes += bytes;
            }
        }
    }

    // Only set values if we found relevant nodes
    if has_any_scan {
        if total_raw_rows > 0 {
            stats.raw_rows_read = Some(total_raw_rows);
        }
        if total_bytes > 0 {
            stats.bytes_read = Some(total_bytes);
        }
        if total_pages_memory > 0 {
            stats.pages_count_memory = Some(total_pages_memory);
        }
        if total_pages_local > 0 {
            stats.pages_count_local_disk = Some(total_pages_local);
        }
        if total_pages_remote > 0 {
            stats.pages_count_remote = Some(total_pages_remote);
        }
        if total_io_seek_ms > 0.0 {
            stats.io_seek_time_ms = Some(total_io_seek_ms);
        }
        if total_local_io_ms > 0.0 {
            stats.local_disk_read_io_time_ms = Some(total_local_io_ms);
        }
        if total_remote_io_ms > 0.0 {
            stats.remote_read_io_time_ms = Some(total_remote_io_ms);
        }
    }

    if has_any_sink {
        if total_result_rows > 0 {
            stats.result_rows = Some(total_result_rows);
        }
        if total_result_bytes > 0 {
            stats.result_bytes = Some(total_result_bytes);
        }
    }

    stats
}
