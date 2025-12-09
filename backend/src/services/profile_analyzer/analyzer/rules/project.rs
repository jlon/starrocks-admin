//! Project and LocalExchange operator rules (P001, P002, L001)
//!
//! P001: Project 表达式计算耗时高
//! P002: 公共子表达式计算耗时高 (CASE WHEN 等重复计算)
//! L001: LocalExchange 内存使用过高

use super::*;

/// P001: Project expression compute time high
pub struct P001ProjectExprHigh;

impl DiagnosticRule for P001ProjectExprHigh {
    fn id(&self) -> &str {
        "P001"
    }
    fn name(&self) -> &str {
        "Project 表达式计算耗时高"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("PROJECT")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let expr_time = context.get_metric("ExprComputeTime")?;
        let op_time = context.get_operator_time_ms()? * 1_000_000.0; // Convert to ns
        if op_time == 0.0 {
            return None;
        }
        let ratio = expr_time / op_time;
        if ratio > 0.5 && expr_time > 100_000_000.0 {
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
                message: format!("Project 表达式计算占比过高 ({:.1}%)", ratio * 100.0),
                reason: "Project 算子执行时间过长，可能是表达式计算复杂或数据量大。".to_string(),
                suggestions: vec![
                    "简化 SELECT 中的复杂表达式".to_string(),
                    "将复杂计算移到物化视图中预计算".to_string(),
                    "检查是否有不必要的类型转换".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// P002: Common sub-expression compute time high (CASE WHEN repeated calculation)
/// Source: project_operator.cpp - CommonSubExprComputeTime
pub struct P002CommonSubExprHigh;

impl DiagnosticRule for P002CommonSubExprHigh {
    fn id(&self) -> &str {
        "P002"
    }
    fn name(&self) -> &str {
        "公共子表达式计算耗时高"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("PROJECT")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // Check CommonSubExprComputeTime
        let common_sub_expr_time = context
            .get_metric("CommonSubExprComputeTime")
            .or_else(|| context.get_metric_duration("CommonSubExprComputeTime"))?;

        // Convert to ms if needed (could be in ns)
        let time_ms = if common_sub_expr_time > 1_000_000.0 {
            common_sub_expr_time / 1_000_000.0 // ns to ms
        } else {
            common_sub_expr_time
        };

        // Threshold: 500ms is significant
        if time_ms < 500.0 {
            return None;
        }

        // Also get ExprComputeTime to calculate ratio
        let expr_time = context
            .get_metric("ExprComputeTime")
            .or_else(|| context.get_metric_duration("ExprComputeTime"))
            .unwrap_or(0.0);
        let expr_time_ms =
            if expr_time > 1_000_000.0 { expr_time / 1_000_000.0 } else { expr_time };

        let total_expr_time = time_ms + expr_time_ms;
        let common_ratio =
            if total_expr_time > 0.0 { time_ms / total_expr_time * 100.0 } else { 0.0 };

        let severity = if time_ms > 5000.0 { RuleSeverity::Error } else { RuleSeverity::Warning };

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
                "公共子表达式计算耗时 {}（占表达式总时间 {:.1}%）",
                format_duration_ms(time_ms), common_ratio
            ),
            reason: "复杂 CASE WHEN 表达式或重复子表达式导致计算开销高。StarRocks 会尝试提取公共子表达式以避免重复计算，但提取本身也有开销。".to_string(),
            suggestions: vec![
                "简化 CASE WHEN 表达式，减少分支数量".to_string(),
                "将复杂条件判断移到物化视图预计算".to_string(),
                "检查是否存在大量重复的表达式计算".to_string(),
                "考虑使用 IF() 替代简单的 CASE WHEN".to_string(),
            ],
            parameter_suggestions: vec![],
                threshold_metadata: None,
        })
    }
}

/// L001: LocalExchange memory too high
pub struct L001LocalExchangeMemory;

impl DiagnosticRule for L001LocalExchangeMemory {
    fn id(&self) -> &str {
        "L001"
    }
    fn name(&self) -> &str {
        "LocalExchange 内存使用过高"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("LOCAL")
            && node.operator_name.to_uppercase().contains("EXCHANGE")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let memory = context
            .get_metric("LocalExchangePeakMemoryUsage")
            .or_else(|| context.get_memory_usage().map(|v| v as f64))?;
        const ONE_GB: f64 = 1024.0 * 1024.0 * 1024.0;
        if memory > ONE_GB {
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
                message: format!("LocalExchange 内存使用 {}", format_bytes(memory as u64)),
                reason: "请参考 StarRocks 官方文档了解更多信息。".to_string(),
                suggestions: vec![
                    "检查上下游算子的数据流是否平衡".to_string(),
                    "调整 pipeline_dop 参数".to_string(),
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

pub fn get_rules() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(P001ProjectExprHigh),
        Box::new(P002CommonSubExprHigh),
        Box::new(L001LocalExchangeMemory),
    ]
}
