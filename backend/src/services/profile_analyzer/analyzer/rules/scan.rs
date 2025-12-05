//! Scan operator diagnostic rules (S001-S011)
//!
//! Rules for OLAP_SCAN and CONNECTOR_SCAN operators.

use super::*;

/// S001: Data skew detection
/// Condition: max(RowsRead)/avg(RowsRead) > 2
pub struct S001DataSkew;

impl DiagnosticRule for S001DataSkew {
    fn id(&self) -> &str { "S001" }
    fn name(&self) -> &str { "Scan 数据倾斜" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // Check for max/min rows read metrics
        let max_rows = context.get_metric("__MAX_OF_RowsRead")
            .or_else(|| context.get_metric("RowsRead"))?;
        let min_rows = context.get_metric("__MIN_OF_RowsRead").unwrap_or(0.0);
        
        if min_rows == 0.0 {
            return None;
        }
        
        let avg_rows = (max_rows + min_rows) / 2.0;
        let ratio = max_rows / avg_rows;
        
        if ratio > 2.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "Scan 存在数据倾斜，max/avg 比率为 {:.2}",
                    ratio
                ),
                reason: "StarRocks 数据在各个存储节点分布不均，使得某些节点在读取数据时需要扫描更多的数据，导致查询延迟。通常是分桶键选择不当导致数据分布不均匀。".to_string(),
                suggestions: vec![
                    "检查分桶键选择是否合理".to_string(),
                    "考虑重新分桶以均匀分布数据".to_string(),
                    "检查是否存在热点数据".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// S003: Poor filter effectiveness
/// Condition: RowsRead/RawRowsRead > 0.8 (less than 20% filtered)
pub struct S003PoorFilter;

impl DiagnosticRule for S003PoorFilter {
    fn id(&self) -> &str { "S003" }
    fn name(&self) -> &str { "过滤效果差" }
    
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
        
        // Only trigger if we're reading a significant amount of data
        if ratio > 0.8 && raw_rows_read > 100_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "过滤效果差，仅过滤了 {:.1}% 的数据 (读取 {:.0} 行 / 原始 {:.0} 行)",
                    (1.0 - ratio) * 100.0, rows_read, raw_rows_read
                ),
                reason: "基于扫描原始数据量以及最终输出数据量判断，Scan 算子扫描数据量较大但输出给下游的数据量未显著减少。StarRocks 提供索引、谓词下推、Runtime Filter 等多种方式过滤数据。".to_string(),
                suggestions: vec![
                    "添加更精确的过滤条件".to_string(),
                    "检查谓词是否可以下推到存储层".to_string(),
                    "考虑使用分区裁剪".to_string(),
                    "检查 ZoneMap 索引是否生效".to_string(),
                ],
                parameter_suggestions: vec![],
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
    fn id(&self) -> &str { "S007" }
    fn name(&self) -> &str { "冷存储访问" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let io_time = context.get_metric("IOTime")
            .or_else(|| context.get_metric("IOTaskExecTime"))?;
        let scan_time = context.get_metric("ScanTime")
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
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "storage_page_cache_limit".to_string(),
                        param_type: ParameterType::BE,
                        current: None,
                        recommended: "30%".to_string(),
                        command: "# 修改 be.conf: storage_page_cache_limit = 30%".to_string(),
                    },
                    ParameterSuggestion {
                        name: "io_tasks_per_scan_operator".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "8".to_string(),
                        command: "SET io_tasks_per_scan_operator = 8;".to_string(),
                    },
                ],
            })
        } else {
            None
        }
    }
}

/// S009: Low cache hit rate
/// Condition: CachedPagesNum/ReadPagesNum < 0.3
pub struct S009LowCacheHit;

impl DiagnosticRule for S009LowCacheHit {
    fn id(&self) -> &str { "S009" }
    fn name(&self) -> &str { "缓存命中率低" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let cached_pages = context.get_metric("CachedPagesNum")?;
        let read_pages = context.get_metric("ReadPagesNum")?;
        
        if read_pages == 0.0 {
            return None;
        }
        
        let hit_rate = cached_pages / read_pages;
        
        if hit_rate < 0.3 && read_pages > 1000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "缓存命中率仅 {:.1}% ({:.0}/{:.0} pages)",
                    hit_rate * 100.0, cached_pages, read_pages
                ),
                reason: "数据缓存命中率低，大量数据需要从磁盘或远程存储读取。可能是缓存容量不足或数据访问模式不适合缓存。".to_string(),
                suggestions: vec![
                    "增大 PageCache 容量".to_string(),
                    "检查是否有其他查询竞争缓存".to_string(),
                    "考虑启用数据缓存".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "enable_scan_datacache".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "true".to_string(),
                        command: "SET enable_scan_datacache = true;".to_string(),
                    },
                ],
            })
        } else {
            None
        }
    }
}

/// S010: Runtime Filter not effective on Scan
/// Condition: RuntimeFilterRows == 0 && RawRowsRead > 100k
pub struct S010RFNotEffective;

impl DiagnosticRule for S010RFNotEffective {
    fn id(&self) -> &str { "S010" }
    fn name(&self) -> &str { "Scan Runtime Filter 未生效" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let rf_rows = context.get_metric("RuntimeFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);
        
        if rf_rows == 0.0 && raw_rows > 100_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "Runtime Filter 未过滤任何行，扫描了 {:.0} 行",
                    raw_rows
                ),
                reason: "Runtime Filter 未能有效过滤数据。可能是 Filter 构建失败、超时或 Filter 选择性差。".to_string(),
                suggestions: vec![
                    "检查 Runtime Filter 是否被生成".to_string(),
                    "检查 Join 条件是否适合生成 RF".to_string(),
                    "确认 enable_global_runtime_filter 已启用".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "enable_global_runtime_filter".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "true".to_string(),
                        command: "SET enable_global_runtime_filter = true;".to_string(),
                    },
                ],
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
    fn id(&self) -> &str { "S011" }
    fn name(&self) -> &str { "累积软删除过多" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let del_vec_rows = context.get_metric("DelVecFilterRows")?;
        let raw_rows = context.get_metric("RawRowsRead")?;
        
        if raw_rows == 0.0 {
            return None;
        }
        
        let ratio = del_vec_rows / raw_rows;
        
        if ratio > 0.3 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "软删除行占比 {:.1}%，建议执行 Compaction",
                    ratio * 100.0
                ),
                reason: "表中存在大量软删除记录，扫描时需要过滤这些已删除的行。建议执行 Compaction 清理删除标记。".to_string(),
                suggestions: vec![
                    "执行手动 Compaction 清理软删除".to_string(),
                    "检查 Compaction 调度是否正常".to_string(),
                    "考虑调整 Compaction 策略".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// S002: IO skew detection
pub struct S002IOSkew;

impl DiagnosticRule for S002IOSkew {
    fn id(&self) -> &str { "S002" }
    fn name(&self) -> &str { "Scan IO 倾斜" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let max_io = context.get_metric("__MAX_OF_IOTime")?;
        let min_io = context.get_metric("__MIN_OF_IOTime").unwrap_or(0.0);
        if min_io == 0.0 { return None; }
        let ratio = max_io / ((max_io + min_io) / 2.0);
        if ratio > 2.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!("Scan IO 耗时存在倾斜，max/avg 比率为 {:.2}", ratio),
                reason: "Scan 算子多个实例在读取数据时，部分实例花费的时间显著大于其它实例。可能是节点 IO 使用率不均或数据在节点上分布不均。".to_string(),
                suggestions: vec!["检查节点 IO 使用率是否不均".to_string(), "检查存储设备是否存在性能问题".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// S004: Predicate not pushed down
pub struct S004PredicateNotPushed;

impl DiagnosticRule for S004PredicateNotPushed {
    fn id(&self) -> &str { "S004" }
    fn name(&self) -> &str { "谓词未下推" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let pushdown = context.get_metric("PushdownPredicates").unwrap_or(0.0);
        let pred_filter = context.get_metric("PredFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);
        if pushdown == 0.0 && raw_rows > 10000.0 && pred_filter / raw_rows > 0.1 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!("谓词未能下推到存储层，{:.0} 行 ({:.1}%) 在表达式层过滤", pred_filter, pred_filter / raw_rows * 100.0),
                reason: "查询条件未能下推到存储层执行，导致需要在计算层过滤大量数据。可能是查询条件包含函数、类型不匹配或不支持下推的表达式。".to_string(),
                suggestions: vec!["将谓词重写为简单比较".to_string(), "添加 zonemap/Bloom 索引".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// S005: IO thread pool saturation
pub struct S005IOThreadPoolSaturation;

impl DiagnosticRule for S005IOThreadPoolSaturation {
    fn id(&self) -> &str { "S005" }
    fn name(&self) -> &str { "IO 线程池饱和" }
    
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
            })
        } else { None }
    }
}

/// S006: Rowset fragmentation
pub struct S006RowsetFragmentation;

impl DiagnosticRule for S006RowsetFragmentation {
    fn id(&self) -> &str { "S006" }
    fn name(&self) -> &str { "Rowset 碎片化" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let rowsets = context.get_metric("RowsetsReadCount").unwrap_or(0.0);
        let init_time = context.get_metric("SegmentInitTime").unwrap_or(0.0);
        if rowsets > 100.0 && init_time > 500_000_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!("Rowset 数量过多 ({:.0})，初始化耗时 {:.1}ms", rowsets, init_time / 1_000_000.0),
                reason: "Rowset 数量过多导致 Segment 初始化时间过长。通常是频繁小批量导入或 Compaction 不及时导致。".to_string(),
                suggestions: vec!["触发手动 Compaction".to_string(), "批量合并小型导入任务".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// S008: ZoneMap index not effective
pub struct S008ZoneMapNotEffective;

impl DiagnosticRule for S008ZoneMapNotEffective {
    fn id(&self) -> &str { "S008" }
    fn name(&self) -> &str { "ZoneMap 索引未生效" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let zonemap_rows = context.get_metric("ZoneMapIndexFilterRows").unwrap_or(0.0);
        let raw_rows = context.get_metric("RawRowsRead").unwrap_or(0.0);
        if zonemap_rows == 0.0 && raw_rows > 100000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: "ZoneMap 索引未能过滤数据".to_string(),
                reason: "ZoneMap 索引未能有效过滤数据。ZoneMap 基于排序键的 min/max 值过滤，需要查询条件包含排序键或前缀列。".to_string(),
                suggestions: vec!["确保查询条件包含排序键或前缀列".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// S012: Bitmap index not effective
/// Condition: BitmapIndexFilterRows = 0 with low cardinality column filter
/// 
/// Reason: Bitmap索引适用于基数较低且大量重复的字段（如性别、状态）。
/// 如果查询条件包含这类字段但未命中Bitmap索引，可能是：
/// 1. 未创建Bitmap索引
/// 2. 查询条件不支持Bitmap索引（如范围查询）
/// 3. 优化器选择了其他索引
pub struct S012BitmapIndexNotEffective;

impl DiagnosticRule for S012BitmapIndexNotEffective {
    fn id(&self) -> &str { "S012" }
    fn name(&self) -> &str { "Bitmap 索引未生效" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
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
    fn id(&self) -> &str { "S013" }
    fn name(&self) -> &str { "Bloom Filter 索引未生效" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("SCAN")
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
    fn id(&self) -> &str { "S014" }
    fn name(&self) -> &str { "可优化为 Colocate Join" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        let name = node.operator_name.to_uppercase();
        name.contains("HASH") && name.contains("JOIN") && name.contains("SHUFFLE")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // Check if this is a shuffle join with significant network transfer
        let bytes_sent = context.get_metric("BytesSent")
            .or_else(|| context.get_metric("NetworkBytesSent")).unwrap_or(0.0);
        
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
            })
        } else {
            None
        }
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
    ]
}
