//! Diagnostic rules module
//!
//! Implements the rule engine for Query Profile diagnostics.
//! Rules are organized by operator type following the design document.

pub mod common;
pub mod scan;
pub mod join;
pub mod aggregate;
pub mod sort;
pub mod exchange;
pub mod query;
pub mod fragment;
pub mod project;
pub mod sink;

use crate::services::profile_analyzer::models::*;

// ============================================================================
// Rule Trait and Types
// ============================================================================

/// Severity level for diagnostic rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
}

impl From<RuleSeverity> for HotSeverity {
    fn from(severity: RuleSeverity) -> Self {
        match severity {
            RuleSeverity::Info => HotSeverity::Mild,
            RuleSeverity::Warning => HotSeverity::Moderate,
            RuleSeverity::Error => HotSeverity::Severe,
        }
    }
}

/// Parameter suggestion for tuning
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParameterSuggestion {
    /// Parameter name (e.g., "enable_scan_datacache")
    pub name: String,
    /// Parameter type (Session or BE)
    pub param_type: ParameterType,
    /// Current value if set, None if using default
    pub current: Option<String>,
    /// Recommended value
    pub recommended: String,
    /// SQL command to set the parameter
    pub command: String,
    /// Human-readable description of what this parameter does
    pub description: String,
    /// Expected impact of changing this parameter
    pub impact: String,
}

/// Parameter type classification
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ParameterType {
    Session,
    BE,
}

/// Get parameter metadata (description and impact) for common StarRocks parameters
pub fn get_parameter_metadata(name: &str) -> ParameterMetadata {
    match name {
        // ========== DataCache 相关 ==========
        "enable_scan_datacache" => ParameterMetadata {
            description: "启用 DataCache 读取缓存，允许从本地缓存读取数据而非远程存储".to_string(),
            impact: "提升存算分离架构下的查询性能，减少网络 IO".to_string(),
        },
        "enable_populate_datacache" => ParameterMetadata {
            description: "启用 DataCache 写入填充，将远程读取的数据缓存到本地".to_string(),
            impact: "后续查询可命中本地缓存，但首次查询会有额外写入开销".to_string(),
        },
        "datacache_evict_probability" => ParameterMetadata {
            description: "DataCache 淘汰概率 (0-100)，控制缓存数据被淘汰的可能性".to_string(),
            impact: "降低该值可减少缓存抖动，但可能导致缓存空间不足".to_string(),
        },
        
        // ========== 查询优化相关 ==========
        "enable_query_cache" => ParameterMetadata {
            description: "启用查询结果缓存，相同查询可直接返回缓存结果".to_string(),
            impact: "对重复查询有显著加速，但会占用额外内存".to_string(),
        },
        "enable_adaptive_sink_dop" => ParameterMetadata {
            description: "启用自适应 Sink 并行度，根据数据量动态调整写入并行度".to_string(),
            impact: "可优化数据写入性能，减少小文件产生".to_string(),
        },
        "enable_runtime_adaptive_dop" => ParameterMetadata {
            description: "启用运行时自适应并行度，根据实际数据量动态调整执行并行度".to_string(),
            impact: "可优化资源利用率，避免小数据量查询占用过多资源".to_string(),
        },
        "enable_spill" => ParameterMetadata {
            description: "启用中间结果落盘，当内存不足时将数据写入磁盘".to_string(),
            impact: "可处理超大数据量查询，但会降低查询性能".to_string(),
        },
        
        // ========== 扫描优化相关 ==========
        "enable_connector_adaptive_io_tasks" => ParameterMetadata {
            description: "启用连接器自适应 IO 任务数，根据数据量动态调整 IO 并行度".to_string(),
            impact: "可优化外部表扫描性能，平衡 IO 和 CPU 资源".to_string(),
        },
        "io_tasks_per_scan_operator" => ParameterMetadata {
            description: "每个扫描算子的 IO 任务数，控制本地表扫描并行度".to_string(),
            impact: "增大可提升扫描吞吐，但会增加 IO 压力".to_string(),
        },
        "connector_io_tasks_per_scan_operator" => ParameterMetadata {
            description: "每个连接器扫描算子的 IO 任务数，控制外部表扫描并行度".to_string(),
            impact: "增大可提升外部表扫描吞吐，但会增加远程存储压力".to_string(),
        },
        
        // ========== Join 优化相关 ==========
        "hash_join_push_down_right_table" => ParameterMetadata {
            description: "启用 Hash Join 右表下推，将小表广播到各节点".to_string(),
            impact: "可减少数据 Shuffle，提升 Join 性能".to_string(),
        },
        "enable_local_shuffle_agg" => ParameterMetadata {
            description: "启用本地 Shuffle 聚合，在本地先进行预聚合".to_string(),
            impact: "可减少网络传输数据量，提升聚合性能".to_string(),
        },
        
        // ========== Runtime Filter 相关 ==========
        "runtime_filter_on_exchange_node" => ParameterMetadata {
            description: "在 Exchange 节点启用 Runtime Filter，跨节点传递过滤条件".to_string(),
            impact: "可提前过滤数据减少 Shuffle，但会增加 Filter 构建开销".to_string(),
        },
        "global_runtime_filter_build_max_size" => ParameterMetadata {
            description: "全局 Runtime Filter 最大构建大小 (字节)".to_string(),
            impact: "增大可支持更大的 Filter，但会占用更多内存".to_string(),
        },
        
        // ========== 并行执行相关 ==========
        "parallel_fragment_exec_instance_num" => ParameterMetadata {
            description: "每个 Fragment 的并行执行实例数".to_string(),
            impact: "增大可提升并行度，但会占用更多资源".to_string(),
        },
        "pipeline_dop" => ParameterMetadata {
            description: "Pipeline 执行并行度，0 表示自动".to_string(),
            impact: "手动设置可控制资源使用，自动模式根据 CPU 核数调整".to_string(),
        },
        
        // ========== 内存相关 ==========
        "query_mem_limit" => ParameterMetadata {
            description: "单个查询的内存限制 (字节)".to_string(),
            impact: "增大可处理更大数据量，但可能影响其他查询".to_string(),
        },
        "query_timeout" => ParameterMetadata {
            description: "查询超时时间 (秒)".to_string(),
            impact: "增大可允许长时间运行的查询，但可能占用资源过久".to_string(),
        },
        
        // ========== 聚合相关 ==========
        "streaming_preaggregation_mode" => ParameterMetadata {
            description: "流式预聚合模式 (auto/force_streaming/force_preaggregation)".to_string(),
            impact: "auto 模式自动选择最优策略，force 模式强制使用指定策略".to_string(),
        },
        "enable_sort_aggregate" => ParameterMetadata {
            description: "启用排序聚合，适用于高基数 GROUP BY".to_string(),
            impact: "可减少内存使用，但需要额外排序开销".to_string(),
        },
        
        // ========== Profile 相关 ==========
        "pipeline_profile_level" => ParameterMetadata {
            description: "Pipeline Profile 详细级别 (0-2)".to_string(),
            impact: "级别越高信息越详细，但收集开销也越大".to_string(),
        },
        
        // ========== BE 参数 ==========
        "storage_page_cache_limit" => ParameterMetadata {
            description: "BE 存储页缓存大小限制".to_string(),
            impact: "增大可提升热数据读取性能，但会占用更多内存".to_string(),
        },
        
        // ========== 默认 ==========
        _ => ParameterMetadata {
            description: format!("StarRocks 参数 {}", name),
            impact: "请参考 StarRocks 官方文档了解详情".to_string(),
        },
    }
}

/// Metadata for a parameter
#[derive(Debug, Clone)]
pub struct ParameterMetadata {
    pub description: String,
    pub impact: String,
}

impl ParameterSuggestion {
    /// Create a new parameter suggestion with automatic metadata lookup
    pub fn new(name: &str, param_type: ParameterType, current: Option<String>, recommended: &str, command: &str) -> Self {
        let metadata = get_parameter_metadata(name);
        Self {
            name: name.to_string(),
            param_type,
            current,
            recommended: recommended.to_string(),
            command: command.to_string(),
            description: metadata.description,
            impact: metadata.impact,
        }
    }
    
    /// Create a session parameter suggestion
    pub fn session(name: &str, recommended: &str) -> Self {
        let command = format!("SET {} = {};", name, recommended);
        Self::new(name, ParameterType::Session, None, recommended, &command)
    }
    
    /// Create a BE parameter suggestion
    pub fn be(name: &str, recommended: &str) -> Self {
        let command = format!("# 修改 be.conf: {} = {}", name, recommended);
        Self::new(name, ParameterType::BE, None, recommended, &command)
    }
}

/// A diagnostic result from rule evaluation
/// 
/// Structure follows Aliyun EMR StarRocks diagnostic standard:
/// - message: 诊断结果概要说明 (Summary of the issue)
/// - reason: 详细诊断原因说明 (Detailed explanation of why this happens)
/// - suggestions: 建议措施 (Recommended actions)
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: RuleSeverity,
    pub node_path: String,
    /// Plan node ID for associating diagnostic with execution tree node
    pub plan_node_id: Option<i32>,
    /// Summary of the diagnostic issue (诊断结果概要)
    pub message: String,
    /// Detailed explanation of why this issue occurs (详细诊断原因)
    pub reason: String,
    /// Recommended actions to fix the issue (建议措施)
    pub suggestions: Vec<String>,
    pub parameter_suggestions: Vec<ParameterSuggestion>,
}

impl Diagnostic {
    /// Convert to HotSpot for backward compatibility
    pub fn to_hotspot(&self) -> HotSpot {
        let mut all_suggestions = self.suggestions.clone();
        
        // Add parameter suggestions as formatted strings
        for param in &self.parameter_suggestions {
            all_suggestions.push(format!(
                "调整参数: {} → {} (命令: {})",
                param.name, param.recommended, param.command
            ));
        }
        
        HotSpot {
            node_path: self.node_path.clone(),
            severity: self.severity.into(),
            issue_type: self.rule_id.clone(),
            description: self.message.clone(),
            suggestions: all_suggestions,
        }
    }
}

/// Context for rule evaluation
pub struct RuleContext<'a> {
    pub node: &'a ExecutionTreeNode,
    /// Non-default session variables from profile summary
    pub session_variables: &'a std::collections::HashMap<String, SessionVariableInfo>,
}

impl<'a> RuleContext<'a> {
    /// Get a metric value from unique_metrics as f64
    pub fn get_metric(&self, name: &str) -> Option<f64> {
        self.node.unique_metrics.get(name)
            .and_then(|v| parse_metric_value(v))
    }
    
    /// Get operator total time in ms
    pub fn get_operator_time_ms(&self) -> Option<f64> {
        self.node.metrics.operator_total_time.map(|ns| ns as f64 / 1_000_000.0)
    }
    
    /// Get time percentage
    pub fn get_time_percentage(&self) -> Option<f64> {
        self.node.time_percentage
    }
    
    /// Get memory usage in bytes
    pub fn get_memory_usage(&self) -> Option<u64> {
        self.node.metrics.memory_usage
    }
    
    /// Check if a session variable is already set to the expected value
    /// Returns true if the variable is set and matches the expected value
    pub fn is_variable_set_to(&self, name: &str, expected: &str) -> bool {
        self.session_variables.get(name)
            .map(|info| info.actual_value_is(expected))
            .unwrap_or(false)
    }
    
    /// Get current value of a session variable as string, or None if not set
    pub fn get_variable_value(&self, name: &str) -> Option<String> {
        self.session_variables.get(name)
            .map(|info| info.actual_value_str())
    }
    
    /// Create a parameter suggestion only if the parameter is not already set to the recommended value
    /// Returns None if the parameter is already set to the recommended value (no suggestion needed)
    /// 
    /// Note: For parameters not in NonDefaultSessionVariables, we check against known defaults.
    /// If a parameter uses its default value and that default matches the recommendation, no suggestion is made.
    pub fn suggest_parameter(&self, name: &str, recommended: &str, command: &str) -> Option<ParameterSuggestion> {
        // Check if already set to recommended value in non-default variables
        if self.is_variable_set_to(name, recommended) {
            return None; // Already configured correctly, no suggestion needed
        }
        
        // If parameter is not in non_default_variables, check if default value matches recommendation
        if !self.session_variables.contains_key(name) {
            if let Some(default) = get_parameter_default(name) {
                if default.eq_ignore_ascii_case(recommended) {
                    return None; // Using default value which matches recommendation
                }
            }
        }
        
        // Get current value if set
        let current = self.get_variable_value(name);
        
        // Get parameter metadata for description and impact
        let metadata = get_parameter_metadata(name);
        
        Some(ParameterSuggestion {
            name: name.to_string(),
            param_type: ParameterType::Session,
            current,
            recommended: recommended.to_string(),
            command: command.to_string(),
            description: metadata.description,
            impact: metadata.impact,
        })
    }
}

/// Known default values for common StarRocks session parameters
/// This helps avoid suggesting parameters that are already at their recommended default values
fn get_parameter_default(name: &str) -> Option<&'static str> {
    match name {
        // DataCache related
        "enable_scan_datacache" => Some("true"),
        "enable_populate_datacache" => Some("true"),
        "datacache_evict_probability" => Some("100"),
        
        // Query optimization
        "enable_query_cache" => Some("false"),
        "enable_adaptive_sink_dop" => Some("false"),
        "enable_runtime_adaptive_dop" => Some("false"),
        "enable_spill" => Some("false"),
        
        // Scan optimization
        "enable_connector_adaptive_io_tasks" => Some("true"),
        "io_tasks_per_scan_operator" => Some("4"),
        "connector_io_tasks_per_scan_operator" => Some("16"),
        
        // Join optimization
        "hash_join_push_down_right_table" => Some("true"),
        "enable_local_shuffle_agg" => Some("true"),
        
        // Runtime filter
        "runtime_filter_on_exchange_node" => Some("false"),
        "global_runtime_filter_build_max_size" => Some("67108864"),
        
        // Parallel execution
        "parallel_fragment_exec_instance_num" => Some("1"),
        "pipeline_dop" => Some("0"),
        
        _ => None,
    }
}

/// Trait for diagnostic rules
pub trait DiagnosticRule: Send + Sync {
    /// Rule ID (e.g., "S001", "J001")
    fn id(&self) -> &str;
    
    /// Rule name
    fn name(&self) -> &str;
    
    /// Check if rule applies to this node
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool;
    
    /// Evaluate the rule and return diagnostic if triggered
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic>;
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Parse metric value from string (handles various formats)
pub fn parse_metric_value(value: &str) -> Option<f64> {
    let s = value.trim();
    
    // Handle percentage
    if s.ends_with('%') {
        return s.trim_end_matches('%').parse().ok();
    }
    
    // Handle bytes (e.g., "1.5 GB", "100 MB")
    if let Some(bytes) = parse_bytes(s) {
        return Some(bytes as f64);
    }
    
    // Handle time (e.g., "1s500ms", "100ms")
    if let Some(ms) = parse_duration_ms(s) {
        return Some(ms);
    }
    
    // Handle plain numbers with optional suffix
    let numeric_part: String = s.chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    
    numeric_part.parse().ok()
}

/// Parse bytes string to u64
pub fn parse_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split_whitespace().collect();
    
    if parts.len() != 2 {
        return None;
    }
    
    let value: f64 = parts[0].parse().ok()?;
    let unit = parts[1].to_uppercase();
    
    let multiplier = match unit.as_str() {
        "B" => 1u64,
        "KB" | "K" => 1024,
        "MB" | "M" => 1024 * 1024,
        "GB" | "G" => 1024 * 1024 * 1024,
        "TB" | "T" => 1024 * 1024 * 1024 * 1024,
        _ => return None,
    };
    
    Some((value * multiplier as f64) as u64)
}

/// Parse duration string to milliseconds
pub fn parse_duration_ms(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    
    let mut total_ms = 0.0;
    let mut num_buf = String::new();
    let mut found_unit = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' {
            num_buf.push(c);
            i += 1;
        } else {
            let value: f64 = num_buf.parse().unwrap_or(0.0);
            num_buf.clear();
            
            if c == 'h' {
                total_ms += value * 3600.0 * 1000.0;
                found_unit = true;
                i += 1;
            } else if c == 'm' {
                if i + 1 < chars.len() && chars[i + 1] == 's' {
                    total_ms += value;
                    i += 2;
                } else {
                    total_ms += value * 60.0 * 1000.0;
                    i += 1;
                }
                found_unit = true;
            } else if c == 's' {
                total_ms += value * 1000.0;
                found_unit = true;
                i += 1;
            } else if c == 'u' && i + 1 < chars.len() && chars[i + 1] == 's' {
                total_ms += value / 1000.0;
                found_unit = true;
                i += 2;
            } else if c == 'n' && i + 1 < chars.len() && chars[i + 1] == 's' {
                total_ms += value / 1_000_000.0;
                found_unit = true;
                i += 2;
            } else {
                i += 1;
            }
        }
    }
    
    if found_unit { Some(total_ms) } else { None }
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Format duration in ms to human-readable string
pub fn format_duration_ms(ms: f64) -> String {
    if ms < 1.0 {
        format!("{:.2}μs", ms * 1000.0)
    } else if ms < 1000.0 {
        format!("{:.2}ms", ms)
    } else if ms < 60000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else if ms < 3600000.0 {
        format!("{:.1}m", ms / 60000.0)
    } else {
        format!("{:.1}h", ms / 3600000.0)
    }
}

// ============================================================================
// Rule Registry
// ============================================================================

/// Get all registered rules
pub fn get_all_rules() -> Vec<Box<dyn DiagnosticRule>> {
    let mut rules: Vec<Box<dyn DiagnosticRule>> = Vec::new();
    
    // Common rules (G001, G002, G003)
    rules.extend(common::get_rules());
    
    // Scan rules (S001-S011)
    rules.extend(scan::get_rules());
    
    // Join rules (J001-J010)
    rules.extend(join::get_rules());
    
    // Aggregate rules (A001-A005)
    rules.extend(aggregate::get_rules());
    
    // Sort rules (T001-T005, W001)
    rules.extend(sort::get_rules());
    
    // Exchange rules (E001-E003)
    rules.extend(exchange::get_rules());
    
    // Fragment rules (F001-F003)
    rules.extend(fragment::get_rules());
    
    // Project/LocalExchange rules (P001, L001)
    rules.extend(project::get_rules());
    
    // OlapTableSink rules (I001-I003)
    rules.extend(sink::get_rules());
    
    // Query rules (Q001-Q009) - evaluated separately at query level
    
    rules
}

/// Get query-level rules
pub fn get_query_rules() -> Vec<Box<dyn query::QueryRule>> {
    query::get_rules()
}
