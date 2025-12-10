//! Planner phase diagnostic rules
//!
//! Rules for detecting issues in the query planning phase:
//! - PL001: HMS metadata retrieval slow
//! - PL002: Optimizer timeout or slow
//! - PL003: Too many partitions to process

use super::*;
use crate::services::profile_analyzer::models::PlannerInfo;

/// Context for planner rule evaluation
pub struct PlannerRuleContext<'a> {
    pub planner: &'a PlannerInfo,
    pub query_time_ms: f64,
}

/// Trait for planner diagnostic rules
pub trait PlannerDiagnosticRule: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn evaluate(&self, context: &PlannerRuleContext) -> Option<Diagnostic>;
}

// ============================================================================
// PL001: HMS Metadata Retrieval Slow
// ============================================================================

/// PL001: HMS (Hive MetaStore) metadata retrieval is slow
/// Condition: Total HMS time > 2 seconds OR any single call > 1 second
pub struct PL001HMSMetadataSlow;

impl PlannerDiagnosticRule for PL001HMSMetadataSlow {
    fn id(&self) -> &str {
        "PL001"
    }
    fn name(&self) -> &str {
        "HMS 元数据获取慢"
    }

    fn evaluate(&self, context: &PlannerRuleContext) -> Option<Diagnostic> {
        let hms = &context.planner.hms_metrics;

        // Thresholds
        let total_threshold_ms = 2000.0; // 2 seconds total
        let single_threshold_ms = 1000.0; // 1 second for single call

        // Check if any HMS metric exceeds threshold
        let mut slow_calls = Vec::new();

        if hms.get_database_ms > single_threshold_ms {
            slow_calls.push(format!("getDatabase: {}", format_duration_ms(hms.get_database_ms)));
        }
        if hms.get_table_ms > single_threshold_ms {
            slow_calls.push(format!("getTable: {}", format_duration_ms(hms.get_table_ms)));
        }
        if hms.get_partitions_ms > single_threshold_ms {
            slow_calls.push(format!(
                "getPartitionsByNames: {}",
                format_duration_ms(hms.get_partitions_ms)
            ));
        }
        if hms.get_partition_stats_ms > single_threshold_ms {
            slow_calls.push(format!(
                "getPartitionColumnStats: {}",
                format_duration_ms(hms.get_partition_stats_ms)
            ));
        }
        if hms.list_partition_names_ms > single_threshold_ms {
            slow_calls.push(format!(
                "listPartitionNames: {}",
                format_duration_ms(hms.list_partition_names_ms)
            ));
        }
        if hms.list_fs_partitions_ms > single_threshold_ms {
            slow_calls.push(format!(
                "LIST_FS_PARTITIONS: {}",
                format_duration_ms(hms.list_fs_partitions_ms)
            ));
        }

        if hms.total_hms_time_ms < total_threshold_ms && slow_calls.is_empty() {
            return None;
        }

        // Calculate HMS ratio - prefer planner time, fallback to query time
        let (ratio_base, ratio_label) = if context.planner.total_time_ms > 0.0 {
            (context.planner.total_time_ms, "Planner")
        } else if context.query_time_ms > 0.0 {
            (context.query_time_ms, "查询总")
        } else {
            (0.0, "")
        };

        let hms_ratio =
            if ratio_base > 0.0 { hms.total_hms_time_ms / ratio_base * 100.0 } else { 0.0 };

        let severity = if hms.total_hms_time_ms > 5000.0 {
            RuleSeverity::Error
        } else {
            RuleSeverity::Warning
        };

        // Build message with ratio info
        let ratio_info = if ratio_base > 0.0 {
            format!("（占{}时间 {:.1}%）", ratio_label, hms_ratio)
        } else {
            String::new() // No ratio info if we can't calculate it
        };

        let message = if slow_calls.is_empty() {
            format!(
                "HMS 元数据获取总耗时 {}{}",
                format_duration_ms(hms.total_hms_time_ms),
                ratio_info
            )
        } else {
            format!(
                "HMS 元数据获取总耗时 {}{}，慢调用: {}",
                format_duration_ms(hms.total_hms_time_ms),
                ratio_info,
                slow_calls.join(", ")
            )
        };

        Some(Diagnostic {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity,
            node_path: "Planner".to_string(),
            plan_node_id: None,
            message,
            reason: "Hive MetaStore 响应慢会阻塞查询规划，可能是 HMS 服务负载高或网络延迟"
                .to_string(),
            suggestions: vec![
                "检查 HMS 服务状态和负载".to_string(),
                "减少查询涉及的分区数量".to_string(),
                "考虑启用元数据缓存".to_string(),
                "检查网络延迟".to_string(),
            ],
            parameter_suggestions: vec![],
            threshold_metadata: None,
        })
    }
}

// ============================================================================
// PL002: Optimizer Timeout or Slow
// ============================================================================

/// PL002: Query optimizer taking too long
/// Condition: Optimizer time > 5 seconds
pub struct PL002OptimizerSlow;

impl PlannerDiagnosticRule for PL002OptimizerSlow {
    fn id(&self) -> &str {
        "PL002"
    }
    fn name(&self) -> &str {
        "优化器耗时过长"
    }

    fn evaluate(&self, context: &PlannerRuleContext) -> Option<Diagnostic> {
        let optimizer_ms = context.planner.optimizer_time_ms;

        // Threshold: 5 seconds
        if optimizer_ms < 5000.0 {
            return None;
        }

        let severity =
            if optimizer_ms > 30000.0 { RuleSeverity::Error } else { RuleSeverity::Warning };

        // Calculate optimizer ratio
        let ratio = if context.planner.total_time_ms > 0.0 {
            optimizer_ms / context.planner.total_time_ms * 100.0
        } else {
            0.0
        };

        Some(Diagnostic {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity,
            node_path: "Planner".to_string(),
            plan_node_id: None,
            message: format!(
                "优化器耗时 {}（占 Planner 时间 {:.1}%）",
                format_duration_ms(optimizer_ms),
                ratio
            ),
            reason: "查询优化器花费过长时间，可能是查询过于复杂或统计信息不准确".to_string(),
            suggestions: vec![
                "简化查询结构，减少 JOIN 和子查询数量".to_string(),
                "更新表的统计信息: ANALYZE TABLE".to_string(),
                "检查 new_planner_optimize_timeout 参数设置".to_string(),
                "考虑拆分为多个简单查询".to_string(),
            ],
            parameter_suggestions: vec![],
            threshold_metadata: None,
        })
    }
}

// ============================================================================
// PL003: High Planner Time Ratio
// ============================================================================

/// PL003: Planner time is high compared to query time
/// Condition: Planner time > 10% of total query time AND > 5 seconds
pub struct PL003HighPlannerRatio;

impl PlannerDiagnosticRule for PL003HighPlannerRatio {
    fn id(&self) -> &str {
        "PL003"
    }
    fn name(&self) -> &str {
        "规划时间占比过高"
    }

    fn evaluate(&self, context: &PlannerRuleContext) -> Option<Diagnostic> {
        let planner_ms = context.planner.total_time_ms;
        let query_ms = context.query_time_ms;

        if query_ms <= 0.0 || planner_ms < 5000.0 {
            return None;
        }

        let ratio = planner_ms / query_ms * 100.0;

        // Only trigger if planner is > 10% of query time
        if ratio < 10.0 {
            return None;
        }

        let severity = if ratio > 30.0 { RuleSeverity::Error } else { RuleSeverity::Warning };

        Some(Diagnostic {
            rule_id: self.id().to_string(),
            rule_name: self.name().to_string(),
            severity,
            node_path: "Planner".to_string(),
            plan_node_id: None,
            message: format!(
                "规划时间 {} 占查询总时间的 {:.1}%",
                format_duration_ms(planner_ms),
                ratio
            ),
            reason: "查询规划占用了大量时间，通常是元数据获取或优化器导致".to_string(),
            suggestions: vec![
                "参见 PL001/PL002 获取具体原因".to_string(),
                "减少查询涉及的表和分区数量".to_string(),
            ],
            parameter_suggestions: vec![],
            threshold_metadata: None,
        })
    }
}

/// Get all planner diagnostic rules
pub fn get_rules() -> Vec<Box<dyn PlannerDiagnosticRule>> {
    vec![
        Box::new(PL001HMSMetadataSlow),
        Box::new(PL002OptimizerSlow),
        Box::new(PL003HighPlannerRatio),
    ]
}

/// Evaluate all planner rules
pub fn evaluate_planner_rules(planner: &PlannerInfo, query_time_ms: f64) -> Vec<Diagnostic> {
    let context = PlannerRuleContext { planner, query_time_ms };
    get_rules()
        .iter()
        .filter_map(|rule| rule.evaluate(&context))
        .collect()
}
