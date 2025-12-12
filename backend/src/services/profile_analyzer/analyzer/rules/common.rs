//! Common diagnostic rules (G001-G003)
//!
//! These rules apply to all operator types.

use super::*;
use crate::services::profile_analyzer::analyzer::thresholds::defaults::{
    MIN_OPERATOR_TIME_MS, MOST_CONSUMING_PERCENTAGE, SECOND_CONSUMING_PERCENTAGE,
};

/// G001: Time percentage too high (most consuming node)
/// Threshold: > 30% AND > 500ms (aligned with StarRocks ExplainAnalyzer.java)
/// P0.2: Added absolute time threshold to avoid false positives on fast operators
pub struct G001MostConsuming;

impl DiagnosticRule for G001MostConsuming {
    fn id(&self) -> &str {
        "G001"
    }
    fn name(&self) -> &str {
        "算子时间占比过高"
    }

    fn applicable_to(&self, _node: &ExecutionTreeNode) -> bool {
        true // Applies to all nodes
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let percentage = context.get_time_percentage()?;
        let operator_time_ms = context.get_operator_time_ms()?;

        // P0.2: Check both percentage AND absolute time threshold
        // Avoid false positives on operators that are fast in absolute terms
        if percentage > MOST_CONSUMING_PERCENTAGE && operator_time_ms > MIN_OPERATOR_TIME_MS {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Error,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "🔴 算子 {} 占用 {:.1}% 的执行时间（最耗时节点）",
                    context.node.operator_name, percentage
                ),
                suggestions: get_operator_suggestions(&context.node.operator_name),
                reason: "算子执行时间占整体查询时间比例过高，是查询的主要瓶颈。优化该算子可获得最大收益。".to_string(),
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// G001b: Time percentage high (second most consuming node)
/// Threshold: > 15% AND > 500ms (aligned with StarRocks ExplainAnalyzer.java)
/// P0.2: Added absolute time threshold to avoid false positives on fast operators
pub struct G001bSecondConsuming;

impl DiagnosticRule for G001bSecondConsuming {
    fn id(&self) -> &str {
        "G001b"
    }
    fn name(&self) -> &str {
        "算子时间占比较高"
    }

    fn applicable_to(&self, _node: &ExecutionTreeNode) -> bool {
        true
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let percentage = context.get_time_percentage()?;
        let operator_time_ms = context.get_operator_time_ms()?;

        // P0.2: Check both percentage AND absolute time threshold
        // Only trigger if between 15% and 30% (G001 handles > 30%)
        if percentage > SECOND_CONSUMING_PERCENTAGE
            && percentage <= MOST_CONSUMING_PERCENTAGE
            && operator_time_ms > MIN_OPERATOR_TIME_MS
        {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "🟠 算子 {} 占用 {:.1}% 的执行时间（次耗时节点）",
                    context.node.operator_name, percentage
                ),
                suggestions: get_operator_suggestions(&context.node.operator_name),
                reason: "算子执行时间占整体查询时间比例过高，是查询的主要瓶颈。优化该算子可获得最大收益。".to_string(),
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// G002: Memory usage too high
/// Threshold: dynamic based on BE memory (10% of BE memory, clamped to 1GB-10GB)
/// v2.0: Uses dynamic memory threshold based on cluster configuration
pub struct G002HighMemory;

impl DiagnosticRule for G002HighMemory {
    fn id(&self) -> &str {
        "G002"
    }
    fn name(&self) -> &str {
        "算子内存使用过高"
    }

    fn applicable_to(&self, _node: &ExecutionTreeNode) -> bool {
        true
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let memory = context.get_memory_usage()?;

        // v2.0: Use dynamic memory threshold based on BE memory
        let memory_threshold = context.thresholds.get_operator_memory_threshold();

        if memory > memory_threshold {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "算子 {} 内存使用过高: {} (阈值: {})",
                    context.node.operator_name, format_bytes(memory), format_bytes(memory_threshold)
                ),
                reason: "算子内存使用过高，可能导致查询失败或触发 Spill。检查是否存在数据膨胀或中间结果过大。".to_string(),
                suggestions: vec![
                    "检查是否存在数据膨胀".to_string(),
                    "考虑分批处理".to_string(),
                    "检查 HashTable 或中间结果是否过大".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = context.suggest_parameter_smart("query_mem_limit") {
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

/// G003: Execution time skew across instances
/// Threshold: max/avg > threshold (dynamic based on cluster size)
/// P0.2: Added absolute value protection (min 500ms execution time)
/// v2.0: Uses dynamic skew threshold based on cluster parallelism
pub struct G003ExecutionSkew;

impl DiagnosticRule for G003ExecutionSkew {
    fn id(&self) -> &str {
        "G003"
    }
    fn name(&self) -> &str {
        "算子执行时间倾斜"
    }

    fn applicable_to(&self, _node: &ExecutionTreeNode) -> bool {
        true
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // Check if we have min/max time metrics
        let max_time = context.node.metrics.operator_total_time_max?;
        let _min_time = context.node.metrics.operator_total_time_min.unwrap_or(0);
        let avg_time = context.node.metrics.operator_total_time?;

        if avg_time == 0 {
            return None;
        }

        // P0.2: Absolute value protection - only check if execution time is significant
        // v2.0: Use constant from thresholds module
        use crate::services::profile_analyzer::analyzer::thresholds::defaults::MIN_EXEC_TIME_NS;
        if avg_time < MIN_EXEC_TIME_NS {
            return None;
        }

        let ratio = max_time as f64 / avg_time as f64;

        // v2.0: Use dynamic skew threshold based on cluster size
        let skew_threshold = context.thresholds.get_skew_threshold();

        if ratio > skew_threshold {
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
                    "算子 {} 存在执行时间倾斜，max/avg 比率为 {:.2} (阈值: {:.1})",
                    context.node.operator_name, ratio, skew_threshold
                ),
                reason:
                    "算子在多个实例间执行时间差异大，部分实例成为瓶颈。通常是数据分布不均匀导致。"
                        .to_string(),
                suggestions: vec![
                    "检查数据分布是否均匀".to_string(),
                    "检查数据分区或分桶是否合理".to_string(),
                    "考虑增加并行度".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = context.suggest_parameter_smart("pipeline_dop") {
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

/// Get operator-specific suggestions based on operator name
fn get_operator_suggestions(operator_name: &str) -> Vec<String> {
    let name = operator_name.to_uppercase();

    if name.contains("SCAN") {
        vec![
            "检查是否可以添加过滤条件减少扫描数据量".to_string(),
            "检查分区裁剪是否生效".to_string(),
            "执行 ANALYZE TABLE 更新统计信息".to_string(),
        ]
    } else if name.contains("JOIN") {
        vec![
            "检查 JOIN 顺序是否最优".to_string(),
            "考虑使用 Runtime Filter".to_string(),
            "检查是否存在数据倾斜".to_string(),
            "执行 ANALYZE TABLE 更新统计信息".to_string(),
        ]
    } else if name.contains("AGGREGATE") || name.contains("AGG") {
        vec![
            "检查聚合模式是否合适".to_string(),
            "考虑使用预聚合或物化视图".to_string(),
            "检查 GROUP BY 键的选择".to_string(),
        ]
    } else if name.contains("EXCHANGE") {
        vec![
            "检查数据分布是否均匀".to_string(),
            "考虑调整并行度".to_string(),
            "检查网络带宽是否充足".to_string(),
        ]
    } else if name.contains("SORT") {
        vec![
            "添加 LIMIT 限制结果集大小".to_string(),
            "检查是否可以使用 Top-N 优化".to_string(),
            "考虑使用物化视图预排序".to_string(),
        ]
    } else {
        vec!["检查该算子是否处理数据量过大".to_string(), "考虑优化查询计划".to_string()]
    }
}

/// Get all common rules
pub fn get_rules() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(G001MostConsuming),
        Box::new(G001bSecondConsuming),
        Box::new(G002HighMemory),
        Box::new(G003ExecutionSkew),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::profile_analyzer::analyzer::thresholds::{DynamicThresholds, QueryType};
    use crate::services::profile_analyzer::models::{
        ExecutionTreeNode, HotSeverity, NodeType, OperatorMetrics,
    };
    use std::collections::HashMap;

    #[test]
    fn test_g001_threshold() {
        let rule = G001MostConsuming;
        assert_eq!(rule.id(), "G001");
    }

    #[test]
    fn test_g001_triggers_on_high_percentage() {
        let rule = G001MostConsuming;

        // Create a node with 99.84% time percentage and sufficient absolute time (1 second)
        // P0.2: G001 now requires both percentage > 30% AND operator_time > 500ms
        let metrics = OperatorMetrics { 
            operator_total_time: Some(1_000_000_000), // 1 second in nanoseconds
            ..Default::default() 
        };

        let node = ExecutionTreeNode {
            id: "test_node".to_string(),
            operator_name: "OLAP_SCAN".to_string(),
            node_type: NodeType::OlapScan,
            plan_node_id: Some(0),
            parent_plan_node_id: None,
            metrics,
            children: vec![],
            depth: 0,
            is_hotspot: false,
            hotspot_severity: HotSeverity::Normal,
            fragment_id: None,
            pipeline_id: None,
            time_percentage: Some(99.84),
            rows: None,
            is_most_consuming: true,
            is_second_most_consuming: false,
            unique_metrics: HashMap::new(),
            has_diagnostic: false,
            diagnostic_ids: vec![],
        };

        let session_variables = std::collections::HashMap::new();
        let context = RuleContext {
            node: &node,
            session_variables: &session_variables,
            cluster_info: None,
            cluster_variables: None,
            default_db: None,
            thresholds: DynamicThresholds::with_defaults(QueryType::Select),
        };
        let result = rule.evaluate(&context);

        assert!(
            result.is_some(),
            "G001 should trigger for 99.84% time percentage with 1s operator time"
        );
        let diag = result.unwrap();
        assert_eq!(diag.rule_id, "G001");
        assert_eq!(diag.plan_node_id, Some(0));
    }

    #[test]
    fn test_g001_skips_fast_operator() {
        let rule = G001MostConsuming;

        // Create a node with high percentage but low absolute time (100ms < 500ms threshold)
        // P0.2: G001 should NOT trigger because operator_time < 500ms
        let metrics = OperatorMetrics { 
            operator_total_time: Some(100_000_000), // 100ms in nanoseconds
            ..Default::default() 
        };

        let node = ExecutionTreeNode {
            id: "test_node".to_string(),
            operator_name: "OLAP_SCAN".to_string(),
            node_type: NodeType::OlapScan,
            plan_node_id: Some(0),
            parent_plan_node_id: None,
            metrics,
            children: vec![],
            depth: 0,
            is_hotspot: false,
            hotspot_severity: HotSeverity::Normal,
            fragment_id: None,
            pipeline_id: None,
            time_percentage: Some(50.0), // High percentage
            rows: None,
            is_most_consuming: true,
            is_second_most_consuming: false,
            unique_metrics: HashMap::new(),
            has_diagnostic: false,
            diagnostic_ids: vec![],
        };

        let session_variables = std::collections::HashMap::new();
        let context = RuleContext {
            node: &node,
            session_variables: &session_variables,
            cluster_info: None,
            cluster_variables: None,
            default_db: None,
            thresholds: DynamicThresholds::with_defaults(QueryType::Select),
        };
        let result = rule.evaluate(&context);

        assert!(
            result.is_none(),
            "G001 should NOT trigger for fast operator (100ms < 500ms threshold)"
        );
    }
}
