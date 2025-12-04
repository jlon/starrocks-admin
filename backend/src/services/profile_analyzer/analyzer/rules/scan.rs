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
                message: format!(
                    "Scan 存在数据倾斜，max/avg 比率为 {:.2}",
                    ratio
                ),
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
                message: format!(
                    "过滤效果差，仅过滤了 {:.1}% 的数据 (读取 {:.0} 行 / 原始 {:.0} 行)",
                    (1.0 - ratio) * 100.0, rows_read, raw_rows_read
                ),
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
                message: format!(
                    "IO 时间占比 {:.1}%，读取数据量 {}，可能存在存储瓶颈",
                    ratio * 100.0, format_bytes(bytes_read as u64)
                ),
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
                message: format!(
                    "缓存命中率仅 {:.1}% ({:.0}/{:.0} pages)",
                    hit_rate * 100.0, cached_pages, read_pages
                ),
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
                message: format!(
                    "Runtime Filter 未过滤任何行，扫描了 {:.0} 行",
                    raw_rows
                ),
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
                message: format!(
                    "软删除行占比 {:.1}%，建议执行 Compaction",
                    ratio * 100.0
                ),
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
                message: format!("Scan IO 耗时存在倾斜，max/avg 比率为 {:.2}", ratio),
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
                message: format!("谓词未能下推到存储层，{:.0} 行 ({:.1}%) 在表达式层过滤", pred_filter, pred_filter / raw_rows * 100.0),
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
                message: format!("IO 线程池可能已饱和，等待时间 {:.1}s", wait_time / 1_000_000_000.0),
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
                message: format!("Rowset 数量过多 ({:.0})，初始化耗时 {:.1}ms", rowsets, init_time / 1_000_000.0),
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
                message: "ZoneMap 索引未能过滤数据".to_string(),
                suggestions: vec!["确保查询条件包含排序键或前缀列".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
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
    ]
}
