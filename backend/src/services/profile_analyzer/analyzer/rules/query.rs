//! Query-level diagnostic rules (Q001-Q009)
//!
//! Rules that evaluate the entire query profile.

use super::{
    ParameterSuggestion, ParameterType, RuleSeverity, format_bytes, format_duration_ms,
    get_parameter_metadata, parse_duration_ms,
};
use crate::services::profile_analyzer::analyzer::thresholds::DynamicThresholds;
use crate::services::profile_analyzer::models::*;

/// Known default values for common StarRocks session parameters
fn get_parameter_default(name: &str) -> Option<&'static str> {
    match name {
        // DataCache related
        "enable_scan_datacache" => Some("true"),
        "enable_populate_datacache" => Some("true"),

        // Query optimization
        "enable_query_cache" => Some("false"),
        "enable_adaptive_sink_dop" => Some("false"),
        "enable_runtime_adaptive_dop" => Some("false"),
        "enable_spill" => Some("false"),

        // Parallelism
        "parallel_fragment_exec_instance_num" => Some("1"),
        "pipeline_dop" => Some("0"),
        "io_tasks_per_scan_operator" => Some("4"),

        _ => None,
    }
}

// Note: suggest_parameter_if_needed is replaced by QueryRuleContext::suggest_parameter

/// Query-level rule context containing all information needed for evaluation
pub struct QueryRuleContext<'a> {
    pub profile: &'a Profile,
    /// Live cluster variables (actual current values from cluster)
    pub cluster_variables: Option<&'a std::collections::HashMap<String, String>>,
    /// Dynamic thresholds (with optional baseline)
    pub thresholds: DynamicThresholds,
}

impl<'a> QueryRuleContext<'a> {
    /// Create QueryRuleContext with explicit thresholds
    ///
    /// This is the only public constructor - forces caller to provide thresholds
    /// which ensures baseline/dynamic threshold support is properly integrated.
    pub fn new(
        profile: &'a Profile,
        cluster_variables: Option<&'a std::collections::HashMap<String, String>>,
        thresholds: DynamicThresholds,
    ) -> Self {
        Self { profile, cluster_variables, thresholds }
    }

    /// Get current value of a parameter
    /// Priority: cluster_variables > non_default_variables > default
    pub fn get_variable_value(&self, name: &str) -> Option<String> {
        // First check live cluster variables (most accurate)
        if let Some(vars) = self.cluster_variables
            && let Some(value) = vars.get(name)
        {
            return Some(value.clone());
        }
        // Then check profile's non-default variables
        if let Some(info) = self.profile.summary.non_default_variables.get(name) {
            return Some(info.actual_value_str());
        }
        // Finally use known default
        get_parameter_default(name).map(|s| s.to_string())
    }

    /// Get current value as i64
    #[allow(dead_code)]
    pub fn get_variable_i64(&self, name: &str) -> Option<i64> {
        self.get_variable_value(name).and_then(|v| v.parse().ok())
    }

    /// Get current value as bool
    #[allow(dead_code)]
    pub fn get_variable_bool(&self, name: &str) -> Option<bool> {
        self.get_variable_value(name)
            .map(|v| v.eq_ignore_ascii_case("true"))
    }

    /// Create a smart parameter suggestion
    /// Returns None if current value already meets recommendation
    pub fn suggest_parameter(&self, name: &str) -> Option<ParameterSuggestion> {
        let cluster_info = self.profile.get_cluster_info();
        let current_str = self.get_variable_value(name);
        let current_i64 = current_str.as_ref().and_then(|v| v.parse::<i64>().ok());
        let current_bool = current_str.as_ref().map(|v| v.eq_ignore_ascii_case("true"));

        let (recommended, reason) = match name {
            "query_timeout" => {
                let current = current_i64.unwrap_or(300);
                if current >= 600 {
                    return None;
                }
                ("600".to_string(), "延长超时时间以支持复杂查询".to_string())
            },

            "query_mem_limit" => {
                let current = current_i64.unwrap_or(0);
                let total_bytes = cluster_info.total_scan_bytes;
                let recommended = if total_bytes > 0 {
                    (total_bytes * 2).clamp(4 * 1024 * 1024 * 1024, 32 * 1024 * 1024 * 1024) as i64
                } else {
                    8 * 1024 * 1024 * 1024
                };
                if current >= recommended {
                    return None;
                }
                let gb = recommended / (1024 * 1024 * 1024);
                (recommended.to_string(), format!("根据数据量推荐 {}GB 内存限制", gb))
            },

            "enable_spill" => {
                if current_bool.unwrap_or(false) {
                    return None;
                }
                ("true".to_string(), "启用后可避免大查询 OOM".to_string())
            },

            "pipeline_profile_level" => {
                let current = current_i64.unwrap_or(1);
                if current <= 1 {
                    return None;
                }
                ("1".to_string(), "降低 Profile 级别减少收集开销".to_string())
            },

            "pipeline_dop" => {
                let current = current_i64.unwrap_or(0);
                if current == 0 {
                    return None;
                }
                ("0".to_string(), "推荐使用自动模式".to_string())
            },

            "enable_scan_datacache" => {
                if current_bool.unwrap_or(true) {
                    return None;
                }
                ("true".to_string(), "启用 DataCache 提升存算分离性能".to_string())
            },

            _ => return None,
        };

        let metadata = get_parameter_metadata(name);
        let command = format!("SET {} = {};", name, recommended);
        Some(ParameterSuggestion {
            name: name.to_string(),
            param_type: ParameterType::Session,
            current: current_str, // Always has value from get_variable_value
            recommended,
            command,
            description: metadata.description,
            impact: format!("{} ({})", metadata.impact, reason),
        })
    }
}

/// Query-level rule trait
pub trait QueryRule: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    /// Evaluate the rule with full context including cluster variables
    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic>;
}

/// Query-level diagnostic result
#[derive(Debug, Clone)]
pub struct QueryDiagnostic {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: RuleSeverity,
    pub message: String,
    pub reason: String,
    pub suggestions: Vec<String>,
    pub parameter_suggestions: Vec<ParameterSuggestion>,
    /// Threshold metadata for traceability
    pub threshold_metadata: Option<super::ThresholdMetadata>,
}

/// Q001: Query execution time too long
/// Condition: TotalTime > threshold (dynamic based on query type)
/// v2.0: Uses query-type-specific time thresholds:
///   - OLAP SELECT: 10s
///   - INSERT/CTAS: 5min
///   - EXPORT/ANALYZE: 10min
///   - LOAD: 30min
///   - Unknown: 1min
pub struct Q001LongRunning;

impl QueryRule for Q001LongRunning {
    fn id(&self) -> &str {
        "Q001"
    }
    fn name(&self) -> &str {
        "查询执行时间过长"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        use crate::services::profile_analyzer::analyzer::thresholds::QueryType;

        let total_time_ms = ctx
            .profile
            .summary
            .total_time_ms
            .or_else(|| parse_duration_ms(&ctx.profile.summary.total_time))?;

        // v3.0: Use adaptive threshold from DynamicThresholds (includes baseline)
        let time_threshold_ms = ctx.thresholds.get_query_time_threshold_ms();
        let has_baseline = ctx.thresholds.baseline.is_some();

        // Format threshold for display
        let threshold_display = if time_threshold_ms >= 60_000.0 {
            format!("{:.0}分钟", time_threshold_ms / 60_000.0)
        } else {
            format!("{:.0}秒", time_threshold_ms / 1000.0)
        };

        // Threshold source for display
        let threshold_source = if has_baseline { "自适应基线" } else { "默认" };

        // Get query type name for display
        let query_type = QueryType::from_sql(&ctx.profile.summary.sql_statement);
        let query_type_name = match query_type {
            QueryType::Select => "OLAP 查询",
            QueryType::Insert => "INSERT 导入",
            QueryType::Export => "EXPORT 导出",
            QueryType::Analyze => "ANALYZE 分析",
            QueryType::Ctas => "CTAS 建表",
            QueryType::Load => "LOAD 导入",
            QueryType::Unknown => "查询",
        };

        if total_time_ms > time_threshold_ms {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!(
                    "{}执行时间 {}，超过{}阈值 ({}, {})",
                    query_type_name,
                    format_duration_ms(total_time_ms),
                    query_type_name,
                    threshold_display,
                    threshold_source
                ),
                reason: if has_baseline {
                    "阈值基于历史基线 P95 + 2σ 计算。当前查询显著慢于同类查询的历史表现。".to_string()
                } else {
                    format!(
                        "根据查询类型 ({}) 使用默认阈值。OLAP 查询期望快速响应 (10s)，而 ETL 任务允许更长时间 (5-30min)。",
                        query_type_name
                    )
                },
                suggestions: vec![
                    "检查是否存在性能瓶颈算子".to_string(),
                    "考虑优化查询计划".to_string(),
                    "检查是否存在数据倾斜".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = ctx.suggest_parameter("query_timeout") {
                        suggestions.push(s);
                    }
                    if let Some(s) = ctx.suggest_parameter("query_mem_limit") {
                        suggestions.push(s);
                    }
                    suggestions
                },
                // Q001 uses adaptive threshold - include metadata for LLM
                threshold_metadata: Some(if has_baseline {
                    let bl = ctx.thresholds.baseline.as_ref().unwrap();
                    super::ThresholdMetadata::from_baseline(time_threshold_ms, bl)
                } else {
                    super::ThresholdMetadata::from_default(time_threshold_ms)
                }),
            })
        } else {
            None
        }
    }
}

/// Q002: Query memory too high
/// Condition: QueryPeakMemory > 10GB
pub struct Q002HighMemory;

impl QueryRule for Q002HighMemory {
    fn id(&self) -> &str {
        "Q002"
    }
    fn name(&self) -> &str {
        "查询内存使用过高"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        let peak_memory = ctx.profile.summary.query_peak_memory?;
        const TEN_GB: u64 = 10 * 1024 * 1024 * 1024;

        if peak_memory > TEN_GB {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!("查询峰值内存 {}，超过 10GB 阈值", format_bytes(peak_memory)),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "检查是否存在大表 Join".to_string(),
                    "考虑启用 Spill 功能".to_string(),
                    "优化查询减少中间结果".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = ctx.suggest_parameter("enable_spill") {
                        suggestions.push(s);
                    }
                    if let Some(s) = ctx.suggest_parameter("query_mem_limit") {
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

/// Q003: Query spill detected
/// Condition: QuerySpillBytes > 0
pub struct Q003QuerySpill;

impl QueryRule for Q003QuerySpill {
    fn id(&self) -> &str {
        "Q003"
    }
    fn name(&self) -> &str {
        "查询发生落盘"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        let spill_bytes_str = ctx.profile.summary.query_spill_bytes.as_ref()?;

        // Parse spill bytes (e.g., "1.5 GB", "0.000 B")
        let spill_bytes = parse_spill_bytes(spill_bytes_str)?;

        if spill_bytes > 0 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                message: format!("查询发生磁盘溢写，溢写数据量 {}", format_bytes(spill_bytes)),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "增加内存限制以减少 Spill".to_string(),
                    "优化查询减少中间结果".to_string(),
                    "检查 Spill 是否影响性能".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = ctx.suggest_parameter("query_mem_limit") {
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

/// Q005: Scan time dominates
/// Condition: ScanTime/TotalTime > 0.8
pub struct Q005ScanDominates;

impl QueryRule for Q005ScanDominates {
    fn id(&self) -> &str {
        "Q005"
    }
    fn name(&self) -> &str {
        "扫描时间占比过高"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        let scan_time_ms = ctx
            .profile
            .summary
            .query_cumulative_scan_time_ms
            .or_else(|| {
                ctx.profile
                    .summary
                    .query_cumulative_scan_time
                    .as_ref()
                    .and_then(|s| parse_duration_ms(s))
            })?;
        let total_time_ms = ctx
            .profile
            .summary
            .total_time_ms
            .or_else(|| parse_duration_ms(&ctx.profile.summary.total_time))?;

        if total_time_ms == 0.0 {
            return None;
        }

        let ratio = scan_time_ms / total_time_ms;

        if ratio > 0.8 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!("扫描时间占比 {:.1}%，查询瓶颈在数据扫描", ratio * 100.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "添加过滤条件减少扫描数据量".to_string(),
                    "检查分区裁剪是否生效".to_string(),
                    "考虑创建物化视图".to_string(),
                    "检查存储性能".to_string(),
                ],
                // Only suggest if not already enabled
                parameter_suggestions: ctx
                    .suggest_parameter("enable_scan_datacache")
                    .into_iter()
                    .collect(),
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// Q006: Network time dominates
/// Condition: NetworkTime/TotalTime > 0.5
pub struct Q006NetworkDominates;

impl QueryRule for Q006NetworkDominates {
    fn id(&self) -> &str {
        "Q006"
    }
    fn name(&self) -> &str {
        "网络时间占比过高"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        let network_time_ms = ctx
            .profile
            .summary
            .query_cumulative_network_time_ms
            .or_else(|| {
                ctx.profile
                    .summary
                    .query_cumulative_network_time
                    .as_ref()
                    .and_then(|s| parse_duration_ms(s))
            })?;
        let total_time_ms = ctx
            .profile
            .summary
            .total_time_ms
            .or_else(|| parse_duration_ms(&ctx.profile.summary.total_time))?;

        if total_time_ms == 0.0 {
            return None;
        }

        let ratio = network_time_ms / total_time_ms;

        if ratio > 0.5 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!("网络时间占比 {:.1}%，查询瓶颈在网络传输", ratio * 100.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "考虑使用 Colocate Join 减少 Shuffle".to_string(),
                    "检查网络带宽".to_string(),
                    "减少跨节点数据传输".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// Q004: CPU utilization low
pub struct Q004LowCPU;

impl QueryRule for Q004LowCPU {
    fn id(&self) -> &str {
        "Q004"
    }
    fn name(&self) -> &str {
        "CPU 利用率低"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        let cpu_time = ctx.profile.summary.query_cumulative_cpu_time_ms?;
        let wall_time = ctx.profile.summary.query_execution_wall_time_ms?;
        if wall_time == 0.0 {
            return None;
        }
        let ratio = cpu_time / wall_time;
        if ratio < 0.3 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: super::RuleSeverity::Warning,
                message: format!("CPU 利用率仅 {:.1}%，可能存在等待或 IO 瓶颈", ratio * 100.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec!["检查是否存在等待".to_string(), "增加并行度".to_string()],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = ctx.suggest_parameter("pipeline_dop") {
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

/// Q007: Profile collection slow
pub struct Q007ProfileCollectSlow;

impl QueryRule for Q007ProfileCollectSlow {
    fn id(&self) -> &str {
        "Q007"
    }
    fn name(&self) -> &str {
        "Profile 收集慢"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        // Check CollectProfileTime from variables or execution metrics
        let collect_time = ctx
            .profile
            .execution
            .metrics
            .get("CollectProfileTime")
            .and_then(|v| v.parse::<f64>().ok())?;
        if collect_time > 100_000_000.0 {
            // 100ms in ns
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: super::RuleSeverity::Info,
                message: format!("Profile 收集时间 {:.1}ms", collect_time / 1_000_000.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec!["降低 pipeline_profile_level".to_string()],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = ctx.suggest_parameter("pipeline_profile_level") {
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

/// Q008: Schedule time too long
pub struct Q008ScheduleTimeLong;

impl QueryRule for Q008ScheduleTimeLong {
    fn id(&self) -> &str {
        "Q008"
    }
    fn name(&self) -> &str {
        "调度时间过长"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        let schedule_time = ctx.profile.summary.query_peak_schedule_time_ms?;
        let wall_time = ctx.profile.summary.query_execution_wall_time_ms?;
        if wall_time == 0.0 {
            return None;
        }
        let ratio = schedule_time / wall_time;
        if ratio > 0.3 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: super::RuleSeverity::Warning,
                message: format!("调度时间占比 {:.1}%，Pipeline 调度可能存在瓶颈", ratio * 100.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec!["检查 Pipeline 调度瓶颈".to_string(), "增加并行度".to_string()],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// Q009: Result delivery slow
pub struct Q009ResultDeliverySlow;

impl QueryRule for Q009ResultDeliverySlow {
    fn id(&self) -> &str {
        "Q009"
    }
    fn name(&self) -> &str {
        "结果传输慢"
    }

    fn evaluate(&self, ctx: &QueryRuleContext) -> Option<QueryDiagnostic> {
        // Check ResultDeliverTime from execution metrics
        let deliver_time = ctx
            .profile
            .execution
            .metrics
            .get("ResultDeliverTime")
            .and_then(|v| v.parse::<f64>().ok())?;
        let wall_time = ctx.profile.summary.query_execution_wall_time_ms? * 1_000_000.0; // to ns
        if wall_time == 0.0 {
            return None;
        }
        let ratio = deliver_time / wall_time;
        if ratio > 0.2 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: super::RuleSeverity::Info,
                message: format!("结果传输时间占比 {:.1}%", ratio * 100.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec!["检查网络带宽".to_string(), "减少结果集大小".to_string()],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// Parse spill bytes string (e.g., "1.5 GB", "0.000 B")
fn parse_spill_bytes(s: &str) -> Option<u64> {
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

/// Get all query-level rules
pub fn get_rules() -> Vec<Box<dyn QueryRule>> {
    vec![
        Box::new(Q001LongRunning),
        Box::new(Q002HighMemory),
        Box::new(Q003QuerySpill),
        Box::new(Q004LowCPU),
        Box::new(Q005ScanDominates),
        Box::new(Q006NetworkDominates),
        Box::new(Q007ProfileCollectSlow),
        Box::new(Q008ScheduleTimeLong),
        Box::new(Q009ResultDeliverySlow),
    ]
}
