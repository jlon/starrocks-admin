//! Scan operator diagnostic rules (S001-S014)
//!
//! Rules for OLAP_SCAN and CONNECTOR_SCAN operators.
//!
//! ## Rule List:
//! - S001: Data skew detection
//! - S002: IO skew detection
//! - S003: Poor filter effectiveness
//! - S004: Predicate not pushed down
//! - S005: IO thread pool saturation
//! - S006: Rowset fragmentation
//! - S007: Cold storage access (IO bound)
//! - S008: ZoneMap index not effective
//! - S009: Low cache hit rate (PageCache + DataCache for disaggregated storage)
//! - S010: Runtime Filter not effective
//! - S011: Accumulated soft deletes
//! - S012: Bitmap index not effective
//! - S013: Bloom filter index not effective
//! - S014: Colocate Join opportunity missed

use super::*;

/// S001: Data skew detection
/// Condition: max(RowsRead)/avg(RowsRead) > threshold (dynamic based on cluster size)
/// Distinguishes between internal tables (bucket key) and external tables (partition skew)
pub struct S001DataSkew;

impl DiagnosticRule for S001DataSkew {
    fn id(&self) -> &str {
        "S001"
    }
    fn name(&self) -> &str {
        "Scan 数据倾斜"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let min_rows = context.get_metric("__MIN_OF_RowsRead").unwrap_or(0.0);
        let max_rows = context
            .get_metric("__MAX_OF_RowsRead")
            .or_else(|| context.get_metric("RowsRead"))?;

        if min_rows == 0.0 && max_rows > 0.0 {
            return None;
        }

        let min_rows_threshold = context.thresholds.get_min_rows_for_skew();
        if max_rows < min_rows_threshold {
            return None;
        }

        let avg_rows = (max_rows + min_rows) / 2.0;
        let ratio = max_rows / avg_rows;
        let skew_threshold = context.thresholds.get_skew_threshold();

        if ratio > skew_threshold {
            let table = context.get_full_table_name();
            let is_internal = context.is_internal_table();

            let (reason, suggestions) = if is_internal {
                (
                    format!(
                        "内表「{}」数据在各节点分布不均（max {:.0} 行，min {:.0} 行）。通常是分桶键选择不当导致数据倾斜。",
                        table, max_rows, min_rows
                    ),
                    vec![
                        format!("检查表「{}」的分桶键是否选择了高基数列", table),
                        format!(
                            "查看数据分布: SELECT COUNT(*) FROM {} GROUP BY <bucket_key> ORDER BY 1 DESC",
                            table
                        ),
                        format!(
                            "必要时重建分桶: ALTER TABLE {} DISTRIBUTED BY HASH(<high_cardinality_column>) BUCKETS N",
                            table
                        ),
                    ],
                )
            } else {
                (
                    format!(
                        "外表「{}」数据在各节点分布不均（max {:.0} 行，min {:.0} 行）。可能是 Hive 分区大小不均或文件分布不均。",
                        table, max_rows, min_rows
                    ),
                    vec![
                        format!("检查外表「{}」的分区数据量是否均衡", table),
                        "检查是否存在热点分区或超大文件".to_string(),
                        "考虑在 Hive 侧重新分区或合并小文件".to_string(),
                    ],
                )
            };

            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!(
                    "{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)
                ),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "Scan 存在数据倾斜，max/avg 比率为 {:.2} (阈值: {:.1})",
                    ratio, skew_threshold
                ),
                reason,
                suggestions,
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S003: Poor filter effectiveness
/// Condition: RowsRead/RawRowsRead > 0.8 (less than 20% filtered)
/// Distinguishes between internal and external tables for suggestions
pub struct S003PoorFilter;

impl DiagnosticRule for S003PoorFilter {
    fn id(&self) -> &str {
        "S003"
    }
    fn name(&self) -> &str {
        "过滤效果差"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let rows_read = context.get_metric("RowsRead")?;
        let raw_rows_read = context.get_metric("RawRowsRead")?;
        if raw_rows_read == 0.0 {
            return None;
        }

        let ratio = rows_read / raw_rows_read;
        let min_rows_threshold = context.thresholds.get_min_rows_for_filter();

        if ratio > 0.8 && raw_rows_read > min_rows_threshold {
            let table = context.get_full_table_name();
            let is_internal = context.is_internal_table();

            let (reason, suggestions) = if is_internal {
                (
                    format!(
                        "内表「{}」扫描了 {:.0} 行但仅过滤掉 {:.1}%。可通过 ZoneMap、BloomFilter 索引或谓词下推提前过滤。",
                        table,
                        raw_rows_read,
                        (1.0 - ratio) * 100.0
                    ),
                    vec![
                        format!("为表「{}」的过滤列添加 ZoneMap 或 BloomFilter 索引", table),
                        "检查 WHERE 条件是否支持下推（避免函数包裹、类型转换）".to_string(),
                        format!("检查表「{}」的分区是否能裁剪", table),
                        "通过 EXPLAIN 查看谓词下推情况".to_string(),
                    ],
                )
            } else {
                (
                    format!(
                        "外表「{}」扫描了 {:.0} 行但仅过滤掉 {:.1}%。外表过滤依赖 Hive 分区裁剪和文件格式的统计信息。",
                        table,
                        raw_rows_read,
                        (1.0 - ratio) * 100.0
                    ),
                    vec![
                        format!(
                            "检查外表「{}」的 Hive 分区是否能裁剪（WHERE 条件包含分区列）",
                            table
                        ),
                        "ORC/Parquet 文件利用 min/max 统计信息过滤，确保文件有 statistics"
                            .to_string(),
                        "检查 WHERE 条件是否支持下推到外部存储".to_string(),
                    ],
                )
            };

            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!(
                    "{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)
                ),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "过滤效果差，仅过滤了 {:.1}% 的数据 (读取 {:.0} 行 / 原始 {:.0} 行)",
                    (1.0 - ratio) * 100.0,
                    rows_read,
                    raw_rows_read
                ),
                reason,
                suggestions,
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S007: Cold storage access (IO bound)
/// Condition: IOTime/ScanTime > 0.8 && BytesRead > 1GB
pub struct S007ColdStorage;

impl DiagnosticRule for S007ColdStorage {
    fn id(&self) -> &str {
        "S007"
    }
    fn name(&self) -> &str {
        "冷存储访问"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let io_time = context
            .get_metric("IOTime")
            .or_else(|| context.get_metric("IOTaskExecTime"))?;
        let scan_time = context
            .get_metric("ScanTime")
            .or_else(|| context.get_operator_time_ms())?;

        if scan_time == 0.0 {
            return None;
        }

        let bytes_read = context.get_metric("BytesRead").unwrap_or(0.0);
        let ratio = io_time / scan_time;

        const ONE_GB: f64 = 1024.0 * 1024.0 * 1024.0;

        if ratio > 0.8 && bytes_read > ONE_GB {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "IO 时间占比 {:.1}%，读取数据量 {}，可能存在存储瓶颈",
                    ratio * 100.0, format_bytes(bytes_read as u64)
                ),
                reason: "数据存储在冷存储（如对象存储）上，IO 延迟较高。冷存储的 IOPS 和吞吐量通常低于本地 SSD。".to_string(),
                suggestions: vec![
                    "检查存储性能，考虑使用 SSD".to_string(),
                    "增大 PageCache 缓存".to_string(),
                    "检查网络带宽（如果是远程存储）".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = context.suggest_parameter_smart("storage_page_cache_limit") {
                        suggestions.push(s);
                    }
                    if let Some(s) = context.suggest_parameter_smart("io_tasks_per_scan_operator") {
                        suggestions.push(s);
                    }
                    suggestions
                },
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S009: Low cache hit rate (PageCache or DataCache)
/// Condition:
/// - PageCache: CachedPagesNum/ReadPagesNum < threshold
/// - DataCache (disaggregated): CompressedBytesReadLocalDisk/(Local+Remote) < threshold
///   v2.0: Uses dynamic cache hit threshold based on storage type
pub struct S009LowCacheHit;

impl DiagnosticRule for S009LowCacheHit {
    fn id(&self) -> &str {
        "S009"
    }
    fn name(&self) -> &str {
        "缓存命中率低"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // v2.0: Get dynamic cache hit threshold
        let cache_threshold = context.thresholds.get_cache_hit_threshold();
        let error_threshold = cache_threshold * 0.6; // Error if below 60% of warning threshold

        // First, check DataCache metrics (for disaggregated storage-compute clusters)
        let bytes_local = context
            .get_metric("CompressedBytesReadLocalDisk")
            .unwrap_or(0.0);
        let bytes_remote = context
            .get_metric("CompressedBytesReadRemote")
            .unwrap_or(0.0);
        let total_bytes = bytes_local + bytes_remote;

        const MIN_BYTES: f64 = 10.0 * 1024.0 * 1024.0; // 10MB

        // DataCache scenario: remote bytes > 0 indicates disaggregated storage
        if bytes_remote > 0.0 && total_bytes > MIN_BYTES {
            let hit_rate = bytes_local / total_bytes;

            // v2.0: Use dynamic threshold
            if hit_rate < cache_threshold {
                let miss_rate = (1.0 - hit_rate) * 100.0;
                return Some(Diagnostic {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: if hit_rate < error_threshold { RuleSeverity::Error } else { RuleSeverity::Warning },
                    node_path: format!("{} (plan_node_id={})", 
                        context.node.operator_name,
                        context.node.plan_node_id.unwrap_or(-1)),
                    plan_node_id: context.node.plan_node_id,
                    message: format!(
                        "DataCache 命中率 {:.1}%，{:.1}% 数据从远程存储读取 (本地: {}, 远程: {})",
                        hit_rate * 100.0,
                        miss_rate,
                        format_bytes(bytes_local as u64),
                        format_bytes(bytes_remote as u64)
                    ),
                    reason: "存算分离架构下，DataCache 是提升查询性能的关键。当大量数据需要从远程存储（如 S3/OSS）读取时，网络延迟会显著影响查询性能。".to_string(),
                    suggestions: vec![
                        "增大 DataCache 容量 (datacache_disk_size)".to_string(),
                        "检查 DataCache 磁盘空间是否充足".to_string(),
                        "对热点数据执行缓存预热 (CACHE SELECT)".to_string(),
                        "检查是否有其他查询竞争缓存资源".to_string(),
                    ],
                    // Only suggest parameters that are not already set to recommended values
                    parameter_suggestions: [
                        context.suggest_parameter(
                            "enable_scan_datacache",
                            "true",
                            "SET enable_scan_datacache = true;"
                        ),
                        context.suggest_parameter(
                            "enable_populate_datacache",
                            "true",
                            "SET enable_populate_datacache = true;"
                        ),
                    ].into_iter().flatten().collect(),
                    threshold_metadata: None,
                });
            }
        }

        // Also check IO count metrics as alternative for DataCache
        let io_local = context.get_metric("IOCountLocalDisk").unwrap_or(0.0);
        let io_remote = context.get_metric("IOCountRemote").unwrap_or(0.0);
        let total_io = io_local + io_remote;

        if io_remote > 0.0 && total_io > 100.0 {
            let hit_rate = io_local / total_io;

            // v2.0: Use dynamic threshold
            if hit_rate < cache_threshold {
                return Some(Diagnostic {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: if hit_rate < error_threshold { RuleSeverity::Error } else { RuleSeverity::Warning },
                    node_path: format!("{} (plan_node_id={})", 
                        context.node.operator_name,
                        context.node.plan_node_id.unwrap_or(-1)),
                    plan_node_id: context.node.plan_node_id,
                    message: format!(
                        "DataCache IO 命中率 {:.1}%，{:.1}% IO 访问远程存储 (本地: {:.0}, 远程: {:.0})",
                        hit_rate * 100.0,
                        (1.0 - hit_rate) * 100.0,
                        io_local,
                        io_remote
                    ),
                    reason: "存算分离架构下，DataCache 是提升查询性能的关键。当大量 IO 请求需要访问远程存储时，网络延迟会显著影响查询性能。".to_string(),
                    suggestions: vec![
                        "增大 DataCache 容量 (datacache_disk_size)".to_string(),
                        "对热点数据执行缓存预热 (CACHE SELECT)".to_string(),
                    ],
                    // Only suggest if not already enabled
                    parameter_suggestions: context.suggest_parameter(
                        "enable_scan_datacache",
                        "true",
                        "SET enable_scan_datacache = true;"
                    ).into_iter().collect(),
                    threshold_metadata: None,
                });
            }
        }

        // Fallback: check PageCache metrics (for shared-nothing clusters)
        let cached_pages = context.get_metric("CachedPagesNum");
        let read_pages = context.get_metric("ReadPagesNum");

        if let (Some(cached), Some(total)) = (cached_pages, read_pages)
            && total > 1000.0
        {
            let hit_rate = cached / total;

            if hit_rate < 0.3 {
                return Some(Diagnostic {
                        rule_id: self.id().to_string(),
                        rule_name: self.name().to_string(),
                        severity: RuleSeverity::Info,
                        node_path: format!("{} (plan_node_id={})", 
                            context.node.operator_name,
                            context.node.plan_node_id.unwrap_or(-1)),
                        plan_node_id: context.node.plan_node_id,
                        message: format!(
                            "PageCache 命中率仅 {:.1}% ({:.0}/{:.0} pages)",
                            hit_rate * 100.0, cached, total
                        ),
                        reason: "PageCache 命中率低，大量数据需要从磁盘读取。可能是缓存容量不足或数据访问模式不适合缓存。".to_string(),
                        suggestions: vec![
                            "增大 PageCache 容量 (storage_page_cache_limit)".to_string(),
                            "检查是否有其他查询竞争缓存".to_string(),
                        ],
                    parameter_suggestions: vec![],
                threshold_metadata: None,
                });
            }
        }

        None
    }
}

/// S010: Runtime Filter not effective on Scan (Internal tables only)
/// Runtime Filter is primarily for internal tables with sorted keys
/// Condition: RuntimeFilterRows == 0 && RawRowsRead > 100k
pub struct S010RFNotEffective;

impl DiagnosticRule for S010RFNotEffective {
    fn id(&self) -> &str {
        "S010"
    }
    fn name(&self) -> &str {
        "Scan Runtime Filter 未生效"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // Runtime Filter is mainly effective on OLAP internal tables
        node.operator_name.to_uppercase().contains("OLAP_SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let rf_rows = context.get_metric("RuntimeFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);

        if rf_rows == 0.0 && raw_rows > 100_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!(
                    "{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)
                ),
                plan_node_id: context.node.plan_node_id,
                message: format!("Runtime Filter 未过滤任何行，扫描了 {:.0} 行", raw_rows),
                reason: {
                    let table = context.get_full_table_name();
                    format!(
                        "表「{}」扫描了 {:.0} 行但 Runtime Filter 未过滤任何数据。可能是 RF 构建失败、超时或选择性差。",
                        table, raw_rows
                    )
                },
                suggestions: {
                    let table = context.get_full_table_name();
                    vec![
                        format!("检查 Join 侧是否生成了针对「{}」的 Runtime Filter", table),
                        "确认 enable_global_runtime_filter = true".to_string(),
                        "检查 RF 是否因 Build 端数据量过大而被跳过".to_string(),
                    ]
                },
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = context.suggest_parameter_smart("enable_global_runtime_filter")
                    {
                        suggestions.push(s);
                    }
                    suggestions
                },
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S011: Accumulated soft deletes
/// Condition: DelVecFilterRows/RawRowsRead > 0.3
pub struct S011SoftDeletes;

impl DiagnosticRule for S011SoftDeletes {
    fn id(&self) -> &str {
        "S011"
    }
    fn name(&self) -> &str {
        "累积软删除过多"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // Only applicable to OLAP_SCAN (internal tables), not CONNECTOR_SCAN (external tables)
        node.operator_name.to_uppercase().contains("OLAP_SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let del_vec_rows = context.get_metric("DelVecFilterRows")?;
        let raw_rows = context.get_metric("RawRowsRead")?;

        if raw_rows == 0.0 {
            return None;
        }

        let ratio = del_vec_rows / raw_rows;

        if ratio > 0.3 {
            // Extract table name from unique_metrics
            let table_name = context
                .node
                .unique_metrics
                .get("Table")
                .map(|s| s.as_str())
                .unwrap_or("unknown_table");

            // Build full table name with database
            // If table already has db prefix (db.table), use it directly
            // Otherwise, use default_db from profile summary
            let full_table_name = if table_name.contains('.') {
                table_name.to_string()
            } else if let Some(db) = context.default_db {
                if db.is_empty() {
                    table_name.to_string()
                } else {
                    format!("{}.{}", db, table_name)
                }
            } else {
                table_name.to_string()
            };

            // Generate Compaction command (correct syntax: ALTER TABLE <table> COMPACT)
            let compaction_cmd = format!("ALTER TABLE {} COMPACT;", full_table_name);

            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!(
                    "{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)
                ),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "表 {} 软删除行占比 {:.1}%，建议执行 Compaction",
                    full_table_name,
                    ratio * 100.0
                ),
                reason: format!(
                    "表 {} 中存在大量软删除记录 ({:.0} 行)，扫描时需要过滤这些已删除的行，影响查询性能。建议执行 Compaction 清理删除标记。",
                    full_table_name, del_vec_rows
                ),
                suggestions: vec![
                    format!("执行 Compaction: {}", compaction_cmd),
                    "检查 Compaction 状态: SHOW PROC '/compactions';".to_string(),
                    format!("查看表 Tablet 状态: SHOW TABLET FROM {};", full_table_name),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S002: IO skew detection
/// Condition: max(IOTime)/avg > threshold (dynamic based on cluster size)
/// P0.2: Added sample protection (min 4 samples) and absolute value protection (min 500ms)
/// v2.0: Uses dynamic skew threshold based on cluster parallelism
pub struct S002IOSkew;

impl DiagnosticRule for S002IOSkew {
    fn id(&self) -> &str {
        "S002"
    }
    fn name(&self) -> &str {
        "Scan IO 倾斜"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // P0.2: Sample protection - need at least 4 instances
        let max_io = context.get_metric("__MAX_OF_IOTime")?;
        let min_io = context.get_metric("__MIN_OF_IOTime").unwrap_or(0.0);

        if min_io == 0.0 {
            return None;
        }

        // P0.2: Absolute value protection - only check if IO time is significant
        // v2.0: Use constant from thresholds module
        use crate::services::profile_analyzer::analyzer::thresholds::defaults::MIN_IO_TIME_NS;

        if max_io < MIN_IO_TIME_NS {
            return None;
        }

        let ratio = max_io / ((max_io + min_io) / 2.0);

        // v2.0: Use dynamic skew threshold based on cluster size
        let skew_threshold = context.thresholds.get_skew_threshold();

        if ratio > skew_threshold {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!("Scan IO 耗时存在倾斜，max/avg 比率为 {:.2} (阈值: {:.1})", ratio, skew_threshold),
                reason: "Scan 算子多个实例在读取数据时，部分实例花费的时间显著大于其它实例。可能是节点 IO 使用率不均或数据在节点上分布不均。".to_string(),
                suggestions: vec!["检查节点 IO 使用率是否不均".to_string(), "检查存储设备是否存在性能问题".to_string()],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S004: Predicate not pushed down
/// Distinguishes internal vs external table suggestions
pub struct S004PredicateNotPushed;

impl DiagnosticRule for S004PredicateNotPushed {
    fn id(&self) -> &str {
        "S004"
    }
    fn name(&self) -> &str {
        "谓词未下推"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let pushdown = context.get_metric("PushdownPredicates").unwrap_or(0.0);
        let pred_filter = context.get_metric("PredFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);

        if pushdown == 0.0 && raw_rows > 10000.0 && pred_filter / raw_rows > 0.1 {
            let is_internal = context.is_internal_table();
            let suggestions = if is_internal {
                vec![
                    "将谓词重写为简单比较（避免函数包裹）".to_string(),
                    "为过滤列添加 ZoneMap/Bloom 索引".to_string(),
                    "检查列类型是否匹配（避免隐式转换）".to_string(),
                ]
            } else {
                vec![
                    "将谓词重写为简单比较（避免函数包裹）".to_string(),
                    "确保过滤列在 Hive 分区列中".to_string(),
                    "检查 ORC/Parquet 文件是否有统计信息".to_string(),
                ]
            };

            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!("谓词未能下推到存储层，{:.0} 行 ({:.1}%) 在表达式层过滤", pred_filter, pred_filter / raw_rows * 100.0),
                reason: "查询条件未能下推到存储层执行，导致需要在计算层过滤大量数据。可能是查询条件包含函数、类型不匹配或不支持下推的表达式。".to_string(),
                suggestions,
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S005: IO thread pool saturation
pub struct S005IOThreadPoolSaturation;

impl DiagnosticRule for S005IOThreadPoolSaturation {
    fn id(&self) -> &str {
        "S005"
    }
    fn name(&self) -> &str {
        "IO 线程池饱和"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let wait_time = context.get_metric("IOTaskWaitTime").unwrap_or(0.0);
        let peak_tasks = context.get_metric("PeakIOTasks").unwrap_or(100.0);
        if wait_time > 1_000_000_000.0 && peak_tasks < 10.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!("IO 线程池可能已饱和，等待时间 {:.1}s", wait_time / 1_000_000_000.0),
                reason: "IO 线程池使用率过高，导致 IO 任务等待时间过长。可能是并发查询过多或存储性能不足。".to_string(),
                suggestions: vec!["增加 BE 上的 max_io_threads 配置".to_string()],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S006: Rowset fragmentation
pub struct S006RowsetFragmentation;

impl DiagnosticRule for S006RowsetFragmentation {
    fn id(&self) -> &str {
        "S006"
    }
    fn name(&self) -> &str {
        "Rowset 碎片化"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // Rowset is a StarRocks internal table concept
        node.operator_name.to_uppercase().contains("OLAP_SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let rowsets = context.get_metric("RowsetsReadCount").unwrap_or(0.0);
        let init_time = context.get_metric("SegmentInitTime").unwrap_or(0.0);
        if rowsets > 100.0 && init_time > 500_000_000.0 {
            let table = context.get_full_table_name();
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!(
                    "{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)
                ),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "Rowset 数量过多 ({:.0})，初始化耗时 {:.1}ms",
                    rowsets,
                    init_time / 1_000_000.0
                ),
                reason: format!(
                    "表「{}」的 Rowset 数量过多（{:.0} 个），导致 Segment 初始化耗时 {:.1}ms。通常是频繁小批量导入或 Compaction 不及时导致。",
                    table,
                    rowsets,
                    init_time / 1_000_000.0
                ),
                suggestions: vec![
                    format!("触发手动 Compaction: ALTER TABLE {} COMPACT", table),
                    format!("查看 Compaction 状态: SHOW TABLET FROM {}", table),
                    "批量合并小型导入任务，减少导入频率".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S008: ZoneMap index not effective (Internal tables only)
/// ZoneMap is a StarRocks internal table feature, not applicable to external tables
pub struct S008ZoneMapNotEffective;

impl DiagnosticRule for S008ZoneMapNotEffective {
    fn id(&self) -> &str {
        "S008"
    }
    fn name(&self) -> &str {
        "ZoneMap 索引未生效"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // ZoneMap is only for internal OLAP tables
        node.operator_name.to_uppercase().contains("OLAP_SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let zonemap_rows = context.get_metric("ZoneMapIndexFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);
        if zonemap_rows == 0.0 && raw_rows > 100000.0 {
            let table = context.get_full_table_name();
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!(
                    "{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)
                ),
                plan_node_id: context.node.plan_node_id,
                message: "ZoneMap 索引未能过滤数据".to_string(),
                reason: format!(
                    "表「{}」扫描了 {:.0} 行但 ZoneMap 未过滤任何数据。ZoneMap 基于排序键的 min/max 值过滤，需要 WHERE 条件包含排序键前缀列。",
                    table, raw_rows
                ),
                suggestions: vec![
                    format!("查看表「{}」的排序键: SHOW CREATE TABLE {}", table, table),
                    "确保 WHERE 条件包含排序键的前缀列（如 WHERE dt = '2024-01-01'）".to_string(),
                    "避免在排序键上使用函数（如 WHERE DATE(dt) = ...）".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S012: Bitmap index not effective (Internal tables only)
/// Condition: BitmapIndexFilterRows = 0 with low cardinality column filter
///
/// Reason: Bitmap索引适用于基数较低且大量重复的字段（如性别、状态）。
/// 如果查询条件包含这类字段但未命中Bitmap索引，可能是：
/// 1. 未创建Bitmap索引
/// 2. 查询条件不支持Bitmap索引（如范围查询）
/// 3. 优化器选择了其他索引
pub struct S012BitmapIndexNotEffective;

impl DiagnosticRule for S012BitmapIndexNotEffective {
    fn id(&self) -> &str {
        "S012"
    }
    fn name(&self) -> &str {
        "Bitmap 索引未生效"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // Bitmap index is only for internal OLAP tables
        node.operator_name.to_uppercase().contains("OLAP_SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let bitmap_rows = context.get_metric("BitmapIndexFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);

        // Only trigger if scanning significant data and bitmap filter is 0
        if bitmap_rows == 0.0 && raw_rows > 100_000.0 {
            // Check if there's expression filter (suggesting there are filter conditions)
            let expr_filter = context.get_metric("ExprFilterRows").unwrap_or(0.0);
            if expr_filter > raw_rows * 0.1 {
                return Some(Diagnostic {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: RuleSeverity::Info,
                    node_path: format!("{} (plan_node_id={})", 
                        context.node.operator_name,
                        context.node.plan_node_id.unwrap_or(-1)),
                    plan_node_id: context.node.plan_node_id,
                    message: format!(
                        "Bitmap 索引未过滤数据，表达式过滤了 {:.0} 行",
                        expr_filter
                    ),
                    reason: "Bitmap 索引适用于基数较低且大量重复的字段（如性别、状态）。如果查询条件包含这类字段但未命中索引，可能是未创建索引或查询条件不支持。".to_string(),
                suggestions: vec![
                        "对低基数列（如状态、类型）创建 Bitmap 索引".to_string(),
                        "确保查询条件使用等值匹配 (=, IN)".to_string(),
                        "检查 Profile 中 BitmapIndexFilterRows 指标".to_string(),
                    ],
                    parameter_suggestions: vec![],
                threshold_metadata: None,
                });
            }
        }
        None
    }
}

/// S013: Bloom filter index not effective
/// Condition: BloomFilterFilterRows = 0 with high cardinality column filter
///
/// Reason: Bloom Filter索引适用于高基数列（如ID列）的等值查询。
/// 如果查询条件包含这类字段但未命中Bloom Filter索引，可能是：
/// 1. 未创建Bloom Filter索引
/// 2. 查询条件不是等值匹配（Bloom Filter仅支持 = 和 IN）
/// 3. 列类型不支持（TINYINT, FLOAT, DOUBLE, DECIMAL不支持）
pub struct S013BloomFilterNotEffective;

impl DiagnosticRule for S013BloomFilterNotEffective {
    fn id(&self) -> &str {
        "S013"
    }
    fn name(&self) -> &str {
        "Bloom Filter 索引未生效"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // Bloom Filter index is only for internal OLAP tables
        node.operator_name.to_uppercase().contains("OLAP_SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let bloom_rows = context.get_metric("BloomFilterFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);

        // Only trigger if scanning significant data and bloom filter is 0
        if bloom_rows == 0.0 && raw_rows > 100_000.0 {
            // Check if there's expression filter on potential ID columns
            let expr_filter = context.get_metric("ExprFilterRows").unwrap_or(0.0);
            if expr_filter > raw_rows * 0.5 {
                return Some(Diagnostic {
                    rule_id: self.id().to_string(),
                    rule_name: self.name().to_string(),
                    severity: RuleSeverity::Info,
                    node_path: format!("{} (plan_node_id={})", 
                        context.node.operator_name,
                        context.node.plan_node_id.unwrap_or(-1)),
                    plan_node_id: context.node.plan_node_id,
                    message: format!(
                        "Bloom Filter 索引未过滤数据，表达式过滤了 {:.0} 行",
                        expr_filter
                    ),
                    reason: "Bloom Filter 索引适用于高基数列（如 ID 列）的等值查询。仅支持 = 和 IN 条件，且 TINYINT/FLOAT/DOUBLE/DECIMAL 类型不支持。".to_string(),
                suggestions: vec![
                        "对高基数列（如 ID 列）创建 Bloom Filter 索引".to_string(),
                        "确保查询条件使用等值匹配 (=, IN)".to_string(),
                        "注意: TINYINT/FLOAT/DOUBLE/DECIMAL 类型不支持 Bloom Filter".to_string(),
                        "检查 Profile 中 BloomFilterFilterRows 指标".to_string(),
                    ],
                    parameter_suggestions: vec![],
                threshold_metadata: None,
                });
            }
        }
        None
    }
}

/// S014: Colocate Join opportunity missed
/// Condition: Shuffle Join on tables that could be colocated
///
/// Reason: Colocate Join可以避免数据网络传输，显著提升Join性能。
/// 当两个表的分桶键相同且分桶数相同时，可以使用Colocate Join。
pub struct S014ColocateJoinOpportunity;

impl DiagnosticRule for S014ColocateJoinOpportunity {
    fn id(&self) -> &str {
        "S014"
    }
    fn name(&self) -> &str {
        "可优化为 Colocate Join"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        let name = node.operator_name.to_uppercase();
        name.contains("HASH") && name.contains("JOIN") && name.contains("SHUFFLE")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // Check if this is a shuffle join with significant network transfer
        let bytes_sent = context
            .get_metric("BytesSent")
            .or_else(|| context.get_metric("NetworkBytesSent"))
            .unwrap_or(0.0);

        const HUNDRED_MB: f64 = 100.0 * 1024.0 * 1024.0;

        if bytes_sent > HUNDRED_MB {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "Shuffle Join 网络传输 {}，考虑使用 Colocate Join 优化",
                    format_bytes(bytes_sent as u64)
                ),
                reason: "Colocate Join 可以避免数据网络传输，显著提升 Join 性能。当两个表的分桶键相同且分桶数相同时，可以使用 Colocate Join。".to_string(),
                suggestions: vec![
                    "将频繁 Join 的表设置为同一 Colocation Group".to_string(),
                    "确保两表的分桶键和分桶数相同".to_string(),
                    "使用 SHOW COLOCATION GROUP 查看现有分组".to_string(),
                    "Colocate Join 可避免数据网络传输，显著提升性能".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S016: Small file detection for external tables (v2.0 updated)
/// Condition: FileCount > threshold AND AvgFileSize < threshold
/// v2.0: Uses ExternalScanType enum for type detection and type-specific suggestions
pub struct S016SmallFiles;

impl DiagnosticRule for S016SmallFiles {
    fn id(&self) -> &str {
        "S016"
    }
    fn name(&self) -> &str {
        "外表小文件过多"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // v2.0: Use ExternalScanType for type detection
        use crate::services::profile_analyzer::analyzer::thresholds::ExternalScanType;
        ExternalScanType::from_operator_name(&node.operator_name)
            .map(|t| t.supports_small_file_detection())
            .unwrap_or(false)
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        use crate::services::profile_analyzer::analyzer::thresholds::{
            ExternalScanType, generate_small_file_suggestions,
        };

        // v2.0: Detect scan type using ExternalScanType enum
        let scan_type = ExternalScanType::from_operator_name(&context.node.operator_name)?;

        // Skip if this type doesn't support small file detection
        if !scan_type.supports_small_file_detection() {
            return None;
        }

        // Get file count using type-specific metric
        let metric_name = scan_type.file_count_metric();
        let file_count = context
            .get_metric(metric_name)
            .or_else(|| context.get_metric("ScanFileCount"))
            .or_else(|| context.get_metric("FileCount"))
            .or_else(|| context.get_metric("TotalFilesNum"))
            .or_else(|| context.get_metric("MorselsCount"))?;

        let total_bytes = context
            .get_metric("BytesRead")
            .or_else(|| context.get_metric("CompressedBytesRead"))?;

        if file_count == 0.0 || total_bytes == 0.0 {
            return None;
        }

        // v2.0: Get storage type from ExternalScanType
        let storage_type = scan_type.storage_type();

        // Get dynamic thresholds based on storage type
        let min_file_count = context.thresholds.get_min_file_count(storage_type) as f64;
        let small_file_threshold = context.thresholds.get_small_file_threshold(storage_type) as f64;

        // Calculate average file size
        let avg_file_size = total_bytes / file_count;

        // Trigger if file count exceeds threshold AND average file size is below threshold
        if file_count > min_file_count && avg_file_size < small_file_threshold {
            // Get table name for suggestions
            let table_name = context
                .node
                .unique_metrics
                .get("Table")
                .map(|s| s.as_str())
                .unwrap_or("external_table");

            // v2.0: Generate type-specific suggestions
            let suggestions = generate_small_file_suggestions(&scan_type, table_name);

            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!(
                    "{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)
                ),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "扫描了 {:.0} 个文件，平均大小仅 {}（建议 > {}）",
                    file_count,
                    format_bytes(avg_file_size as u64),
                    format_bytes(small_file_threshold as u64)
                ),
                reason: format!(
                    "{} 外表 {} 存在大量小文件，导致元数据开销大、IO 效率低。",
                    scan_type.display_name(),
                    table_name
                ),
                suggestions,
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// S017: File format fragmentation detection (ORC Stripes / Parquet RowGroups)
/// Detects when files have too many small stripes/rowgroups, causing excessive IO
/// Distinguishes between internal tables (Compaction) and external tables (Hive merge)
pub struct S017FileFragmentation;

impl DiagnosticRule for S017FileFragmentation {
    fn id(&self) -> &str {
        "S017"
    }
    fn name(&self) -> &str {
        "文件格式碎片化"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        if let Some(diag) = self.check_orc_fragmentation(context) {
            return Some(diag);
        }
        if let Some(diag) = self.check_parquet_fragmentation(context) {
            return Some(diag);
        }
        None
    }
}

impl S017FileFragmentation {
    /// Get suggestions based on table type (internal vs external)
    /// For external tables, provide a single consolidated suggestion to avoid repetition
    fn get_suggestions(is_external: bool, _format: &str, table: &str) -> Vec<String> {
        if is_external {
            // Single consolidated suggestion for external tables
            vec![format!(
                "外表小文件合并方案: ①Hive简单合并: ALTER TABLE {} PARTITION(...) CONCATENATE; \
                 ②推荐重写: INSERT OVERWRITE TABLE {} PARTITION(...) SELECT * FROM {}; \
                 ③大数据量用Spark: df.repartition(N).saveAsTable('{}'); \
                 ④StarRocks临时优化: SET connector_io_tasks_per_scan_operator=64",
                table, table, table, table
            )]
        } else {
            vec![format!("执行 Compaction 合并碎片: ALTER TABLE {} COMPACT", table)]
        }
    }

    fn check_orc_fragmentation(&self, context: &RuleContext) -> Option<Diagnostic> {
        let stripe_count = context.get_metric("TotalStripeNumber")?;
        let stripe_size = context.get_metric_bytes("TotalStripeSize").unwrap_or(0.0);
        let tiny_stripe_size = context
            .get_metric_bytes("TotalTinyStripeSize")
            .unwrap_or(0.0);

        if stripe_count <= 10000.0 {
            return None;
        }

        let avg_stripe_size = if stripe_count > 0.0 { stripe_size / stripe_count } else { 0.0 };
        let tiny_ratio =
            if stripe_size > 0.0 { tiny_stripe_size / stripe_size * 100.0 } else { 0.0 };

        if avg_stripe_size >= 64.0 * 1024.0 * 1024.0 && tiny_ratio < 2.0 {
            return None;
        }

        let table = context
            .node
            .unique_metrics
            .get("Table")
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let is_external = context.is_external_table();
        let severity = if stripe_count > 500000.0 || tiny_ratio > 10.0 {
            RuleSeverity::Error
        } else {
            RuleSeverity::Warning
        };
        let table_type = if is_external { "外表" } else { "内表" };

        Some(Diagnostic {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity,
            node_path: format!(
                "{} (plan_node_id={})",
                context.node.operator_name,
                context.node.plan_node_id.unwrap_or(-1)
            ),
            plan_node_id: context.node.plan_node_id,
            message: format!(
                "ORC 文件存在 {:.0} 个 Stripe（平均 {}），TinyStripe 占比 {:.1}%",
                stripe_count,
                format_bytes(avg_stripe_size as u64),
                tiny_ratio
            ),
            reason: format!(
                "{}「{}」的 ORC 文件 Stripe 碎片化严重（共 {:.0} 个），导致大量 IO 请求和文件打开开销。",
                table_type, table, stripe_count
            ),
            suggestions: Self::get_suggestions(is_external, "ORC", table),
            parameter_suggestions: vec![],
            threshold_metadata: None,
        })
    }

    fn check_parquet_fragmentation(&self, context: &RuleContext) -> Option<Diagnostic> {
        let total_rowgroups = context.get_metric("TotalRowGroups")?;
        if total_rowgroups <= 10000.0 {
            return None;
        }

        let table = context
            .node
            .unique_metrics
            .get("Table")
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let is_external = context.is_external_table();
        let severity =
            if total_rowgroups > 100000.0 { RuleSeverity::Error } else { RuleSeverity::Warning };
        let table_type = if is_external { "外表" } else { "内表" };

        Some(Diagnostic {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity,
            node_path: format!(
                "{} (plan_node_id={})",
                context.node.operator_name,
                context.node.plan_node_id.unwrap_or(-1)
            ),
            plan_node_id: context.node.plan_node_id,
            message: format!("Parquet 文件存在 {:.0} 个 RowGroup，文件碎片化严重", total_rowgroups),
            reason: format!(
                "{}「{}」的 Parquet 文件 RowGroup 过多（共 {:.0} 个），导致元数据开销和 IO 效率低。",
                table_type, table, total_rowgroups
            ),
            suggestions: Self::get_suggestions(is_external, "Parquet", table),
            parameter_suggestions: vec![],
            threshold_metadata: None,
        })
    }
}

/// S018: IO wait time detection for HDFS/external scans
/// Condition: IOTaskWaitTime > threshold (significant IO queue waiting)
pub struct S018IOWaitTime;

impl DiagnosticRule for S018IOWaitTime {
    fn id(&self) -> &str {
        "S018"
    }
    fn name(&self) -> &str {
        "IO 等待时间过长"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        // Applies to any scan node
        let op = node.operator_name.to_uppercase();
        op.contains("SCAN")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // Check IOTaskWaitTime - time spent waiting for IO tasks in queue
        let io_wait = context
            .get_metric_duration("IOTaskWaitTime")
            .or_else(|| context.get_metric_duration("__MAX_OF_IOTaskWaitTime"))?;

        // Also get IOTaskExecTime for comparison
        let io_exec = context
            .get_metric_duration("IOTaskExecTime")
            .or_else(|| context.get_metric_duration("__MAX_OF_IOTaskExecTime"))
            .unwrap_or(0.0);

        // Threshold: IO wait > 10 seconds is significant
        let wait_threshold_ms = 10_000.0;
        if io_wait < wait_threshold_ms {
            return None;
        }

        // Calculate wait ratio
        let total_io = io_wait + io_exec;
        let wait_ratio = if total_io > 0.0 { io_wait / total_io * 100.0 } else { 0.0 };

        let severity = if io_wait > 60_000.0 { RuleSeverity::Error } else { RuleSeverity::Warning };

        Some(Diagnostic {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity,
            node_path: format!(
                "{} (plan_node_id={})",
                context.node.operator_name,
                context.node.plan_node_id.unwrap_or(-1)
            ),
            plan_node_id: context.node.plan_node_id,
            message: format!(
                "IO 等待时间 {}（占 IO 总时间 {:.1}%），存在 IO 排队瓶颈",
                format_duration_ms(io_wait),
                wait_ratio
            ),
            reason: "大量并发 IO 请求在队列中等待，可能是文件碎片化或 IO 资源不足导致".to_string(),
            suggestions: vec![
                "合并小文件减少 IO 请求数".to_string(),
                "增加 io_tasks_per_scan_operator 参数".to_string(),
                "检查存储系统 IO 性能".to_string(),
                "考虑启用 Data Cache 缓存热点数据".to_string(),
            ],
            parameter_suggestions: vec![],
            threshold_metadata: None,
        })
    }
}

/// Get all scan rules
pub fn get_rules() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(S001DataSkew),
        Box::new(S002IOSkew),
        Box::new(S003PoorFilter),
        Box::new(S004PredicateNotPushed),
        Box::new(S005IOThreadPoolSaturation),
        Box::new(S006RowsetFragmentation),
        Box::new(S007ColdStorage),
        Box::new(S008ZoneMapNotEffective),
        Box::new(S009LowCacheHit),
        Box::new(S010RFNotEffective),
        Box::new(S011SoftDeletes),
        Box::new(S012BitmapIndexNotEffective),
        Box::new(S013BloomFilterNotEffective),
        Box::new(S014ColocateJoinOpportunity),
        Box::new(S016SmallFiles),
        Box::new(S017FileFragmentation),
        Box::new(S018IOWaitTime),
    ]
}
