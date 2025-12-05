//! Common diagnostic rules (G001-G003)
//!
//! These rules apply to all operator types.

use super::*;

/// G001: Time percentage too high (most consuming node)
/// Threshold: > 30% (aligned with StarRocks ExplainAnalyzer.java)
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

        if percentage > 30.0 {
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
            })
        } else {
            None
        }
    }
}

/// G001b: Time percentage high (second most consuming node)
/// Threshold: > 15% (aligned with StarRocks ExplainAnalyzer.java)
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

        // Only trigger if between 15% and 30% (G001 handles > 30%)
        if percentage > 15.0 && percentage <= 30.0 {
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
            })
        } else {
            None
        }
    }
}

/// G002: Memory usage too high
/// Threshold: > 1GB
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
        const ONE_GB: u64 = 1024 * 1024 * 1024;

        if memory > ONE_GB {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "算子 {} 内存使用过高: {}",
                    context.node.operator_name, format_bytes(memory)
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
            })
        } else {
            None
        }
    }
}

/// G003: Execution time skew across instances
/// Threshold: max/avg > 2
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

        let ratio = max_time as f64 / avg_time as f64;

        if ratio > 2.0 {
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
                    "算子 {} 存在执行时间倾斜，max/avg 比率为 {:.2}",
                    context.node.operator_name, ratio
                ),
                reason:
                    "算子在多个实例间执行时间差异大，部分实例成为瓶颈。通常是数据分布不均匀导致。"
                        .to_string(),
                suggestions: vec![
                    "检查数据分布是否均匀".to_string(),
                    "检查分桶键选择是否合理".to_string(),
                    "考虑增加并行度".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = context.suggest_parameter_smart("pipeline_dop") {
                        suggestions.push(s);
                    }
                    suggestions
                },
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
            "考虑添加索引或物化视图".to_string(),
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

        // Create a node with 99.84% time percentage
        let node = ExecutionTreeNode {
            id: "test_node".to_string(),
            operator_name: "OLAP_SCAN".to_string(),
            node_type: NodeType::OlapScan,
            plan_node_id: Some(0),
            parent_plan_node_id: None,
            metrics: OperatorMetrics::default(),
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
        };
        let result = rule.evaluate(&context);

        assert!(result.is_some(), "G001 should trigger for 99.84% time percentage");
        let diag = result.unwrap();
        assert_eq!(diag.rule_id, "G001");
        assert_eq!(diag.plan_node_id, Some(0));
    }
}
