//! Query-level diagnostic rules (Q001-Q009)
//!
//! Rules that evaluate the entire query profile.

use crate::services::profile_analyzer::models::*;
use super::{RuleSeverity, ParameterSuggestion, ParameterType, parse_duration_ms, format_bytes, format_duration_ms};

/// Query-level rule trait
pub trait QueryRule: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic>;
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
}

/// Q001: Query execution time too long
/// Condition: TotalTime > 60s
pub struct Q001LongRunning;

impl QueryRule for Q001LongRunning {
    fn id(&self) -> &str { "Q001" }
    fn name(&self) -> &str { "查询执行时间过长" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        let total_time_ms = profile.summary.total_time_ms
            .or_else(|| parse_duration_ms(&profile.summary.total_time))?;
        
        if total_time_ms > 60_000.0 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!(
                    "查询执行时间 {}，超过 60 秒阈值",
                    format_duration_ms(total_time_ms)
                ),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "检查是否存在性能瓶颈算子".to_string(),
                    "考虑优化查询计划".to_string(),
                    "检查是否存在数据倾斜".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "query_timeout".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "600".to_string(),
                        command: "SET query_timeout = 600;".to_string(),
                    },
                    ParameterSuggestion {
                        name: "query_mem_limit".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "8589934592".to_string(),
                        command: "SET query_mem_limit = 8589934592; -- 8GB".to_string(),
                    },
                ],
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
    fn id(&self) -> &str { "Q002" }
    fn name(&self) -> &str { "查询内存使用过高" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        let peak_memory = profile.summary.query_peak_memory?;
        const TEN_GB: u64 = 10 * 1024 * 1024 * 1024;
        
        if peak_memory > TEN_GB {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!(
                    "查询峰值内存 {}，超过 10GB 阈值",
                    format_bytes(peak_memory)
                ),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "检查是否存在大表 Join".to_string(),
                    "考虑启用 Spill 功能".to_string(),
                    "优化查询减少中间结果".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "enable_spill".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "true".to_string(),
                        command: "SET enable_spill = true;".to_string(),
                    },
                    ParameterSuggestion {
                        name: "query_mem_limit".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "17179869184".to_string(),
                        command: "SET query_mem_limit = 17179869184; -- 16GB".to_string(),
                    },
                ],
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
    fn id(&self) -> &str { "Q003" }
    fn name(&self) -> &str { "查询发生落盘" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        let spill_bytes_str = profile.summary.query_spill_bytes.as_ref()?;
        
        // Parse spill bytes (e.g., "1.5 GB", "0.000 B")
        let spill_bytes = parse_spill_bytes(spill_bytes_str)?;
        
        if spill_bytes > 0 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                message: format!(
                    "查询发生磁盘溢写，溢写数据量 {}",
                    format_bytes(spill_bytes)
                ),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "增加内存限制以减少 Spill".to_string(),
                    "优化查询减少中间结果".to_string(),
                    "检查 Spill 是否影响性能".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "query_mem_limit".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "8589934592".to_string(),
                        command: "SET query_mem_limit = 8589934592; -- 8GB".to_string(),
                    },
                ],
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
    fn id(&self) -> &str { "Q005" }
    fn name(&self) -> &str { "扫描时间占比过高" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        let scan_time_ms = profile.summary.query_cumulative_scan_time_ms
            .or_else(|| profile.summary.query_cumulative_scan_time.as_ref()
                .and_then(|s| parse_duration_ms(s)))?;
        let total_time_ms = profile.summary.total_time_ms
            .or_else(|| parse_duration_ms(&profile.summary.total_time))?;
        
        if total_time_ms == 0.0 {
            return None;
        }
        
        let ratio = scan_time_ms / total_time_ms;
        
        if ratio > 0.8 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!(
                    "扫描时间占比 {:.1}%，查询瓶颈在数据扫描",
                    ratio * 100.0
                ),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "添加过滤条件减少扫描数据量".to_string(),
                    "检查分区裁剪是否生效".to_string(),
                    "考虑创建物化视图".to_string(),
                    "检查存储性能".to_string(),
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

/// Q006: Network time dominates
/// Condition: NetworkTime/TotalTime > 0.5
pub struct Q006NetworkDominates;

impl QueryRule for Q006NetworkDominates {
    fn id(&self) -> &str { "Q006" }
    fn name(&self) -> &str { "网络时间占比过高" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        let network_time_ms = profile.summary.query_cumulative_network_time_ms
            .or_else(|| profile.summary.query_cumulative_network_time.as_ref()
                .and_then(|s| parse_duration_ms(s)))?;
        let total_time_ms = profile.summary.total_time_ms
            .or_else(|| parse_duration_ms(&profile.summary.total_time))?;
        
        if total_time_ms == 0.0 {
            return None;
        }
        
        let ratio = network_time_ms / total_time_ms;
        
        if ratio > 0.5 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                message: format!(
                    "网络时间占比 {:.1}%，查询瓶颈在网络传输",
                    ratio * 100.0
                ),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "考虑使用 Colocate Join 减少 Shuffle".to_string(),
                    "检查网络带宽".to_string(),
                    "减少跨节点数据传输".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// Q004: CPU utilization low
pub struct Q004LowCPU;

impl QueryRule for Q004LowCPU {
    fn id(&self) -> &str { "Q004" }
    fn name(&self) -> &str { "CPU 利用率低" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        let cpu_time = profile.summary.query_cumulative_cpu_time_ms?;
        let wall_time = profile.summary.query_execution_wall_time_ms?;
        if wall_time == 0.0 { return None; }
        let ratio = cpu_time / wall_time;
        if ratio < 0.3 {
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: super::RuleSeverity::Warning,
                message: format!("CPU 利用率仅 {:.1}%，可能存在等待或 IO 瓶颈", ratio * 100.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec!["检查是否存在等待".to_string(), "增加并行度".to_string()],
                parameter_suggestions: vec![
                    super::ParameterSuggestion {
                        name: "pipeline_dop".to_string(),
                        param_type: super::ParameterType::Session,
                        current: None,
                        recommended: "0".to_string(),
                        command: "SET pipeline_dop = 0; -- auto".to_string(),
                    },
                ],
            })
        } else { None }
    }
}

/// Q007: Profile collection slow
pub struct Q007ProfileCollectSlow;

impl QueryRule for Q007ProfileCollectSlow {
    fn id(&self) -> &str { "Q007" }
    fn name(&self) -> &str { "Profile 收集慢" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        // Check CollectProfileTime from variables or execution metrics
        let collect_time = profile.execution.metrics.get("CollectProfileTime")
            .and_then(|v| v.parse::<f64>().ok())?;
        if collect_time > 100_000_000.0 { // 100ms in ns
            Some(QueryDiagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: super::RuleSeverity::Info,
                message: format!("Profile 收集时间 {:.1}ms", collect_time / 1_000_000.0),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec!["降低 pipeline_profile_level".to_string()],
                parameter_suggestions: vec![
                    super::ParameterSuggestion {
                        name: "pipeline_profile_level".to_string(),
                        param_type: super::ParameterType::Session,
                        current: None,
                        recommended: "1".to_string(),
                        command: "SET pipeline_profile_level = 1;".to_string(),
                    },
                ],
            })
        } else { None }
    }
}

/// Q008: Schedule time too long
pub struct Q008ScheduleTimeLong;

impl QueryRule for Q008ScheduleTimeLong {
    fn id(&self) -> &str { "Q008" }
    fn name(&self) -> &str { "调度时间过长" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        let schedule_time = profile.summary.query_peak_schedule_time_ms?;
        let wall_time = profile.summary.query_execution_wall_time_ms?;
        if wall_time == 0.0 { return None; }
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
            })
        } else { None }
    }
}

/// Q009: Result delivery slow
pub struct Q009ResultDeliverySlow;

impl QueryRule for Q009ResultDeliverySlow {
    fn id(&self) -> &str { "Q009" }
    fn name(&self) -> &str { "结果传输慢" }
    
    fn evaluate(&self, profile: &Profile) -> Option<QueryDiagnostic> {
        // Check ResultDeliverTime from execution metrics
        let deliver_time = profile.execution.metrics.get("ResultDeliverTime")
            .and_then(|v| v.parse::<f64>().ok())?;
        let wall_time = profile.summary.query_execution_wall_time_ms? * 1_000_000.0; // to ns
        if wall_time == 0.0 { return None; }
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
            })
        } else { None }
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
