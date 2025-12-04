//! Project and LocalExchange operator rules (P001, L001)

use super::*;

/// P001: Project expression compute time high
pub struct P001ProjectExprHigh;

impl DiagnosticRule for P001ProjectExprHigh {
    fn id(&self) -> &str { "P001" }
    fn name(&self) -> &str { "Project 表达式计算耗时高" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("PROJECT")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let expr_time = context.get_metric("ExprComputeTime")?;
        let op_time = context.get_operator_time_ms()? * 1_000_000.0; // Convert to ns
        if op_time == 0.0 { return None; }
        let ratio = expr_time / op_time;
        if ratio > 0.5 && expr_time > 100_000_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("Project 表达式计算占比过高 ({:.1}%)", ratio * 100.0),
                suggestions: vec![
                    "简化 SELECT 中的复杂表达式".to_string(),
                    "将复杂计算移到物化视图中预计算".to_string(),
                    "检查是否有不必要的类型转换".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// L001: LocalExchange memory too high
pub struct L001LocalExchangeMemory;

impl DiagnosticRule for L001LocalExchangeMemory {
    fn id(&self) -> &str { "L001" }
    fn name(&self) -> &str { "LocalExchange 内存使用过高" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("LOCAL") && 
        node.operator_name.to_uppercase().contains("EXCHANGE")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let memory = context.get_metric("LocalExchangePeakMemoryUsage")
            .or_else(|| context.get_memory_usage().map(|v| v as f64))?;
        const ONE_GB: f64 = 1024.0 * 1024.0 * 1024.0;
        if memory > ONE_GB {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("LocalExchange 内存使用 {}", format_bytes(memory as u64)),
                suggestions: vec![
                    "检查上下游算子的数据流是否平衡".to_string(),
                    "调整 pipeline_dop 参数".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "pipeline_dop".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "0".to_string(),
                        command: "SET pipeline_dop = 0; -- auto".to_string(),
                    },
                ],
            })
        } else { None }
    }
}

pub fn get_rules() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(P001ProjectExprHigh),
        Box::new(L001LocalExchangeMemory),
    ]
}
