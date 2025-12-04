//! Aggregate operator diagnostic rules (A001-A005)
//!
//! Rules for AGGREGATE operators.

use super::*;

/// A001: Aggregation skew
/// Condition: max(AggComputeTime)/avg > 2
pub struct A001AggregationSkew;

impl DiagnosticRule for A001AggregationSkew {
    fn id(&self) -> &str { "A001" }
    fn name(&self) -> &str { "聚合数据倾斜" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGG")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let max_time = context.node.metrics.operator_total_time_max?;
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
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "聚合存在数据倾斜，max/avg 比率为 {:.2}",
                    ratio
                ),
                suggestions: vec![
                    "检查 GROUP BY 键的数据分布".to_string(),
                    "考虑使用两阶段聚合".to_string(),
                    "检查是否存在热点键".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// A002: HashTable memory too large
/// Condition: HashTableMemoryUsage > 1GB
pub struct A002HashTableTooLarge;

impl DiagnosticRule for A002HashTableTooLarge {
    fn id(&self) -> &str { "A002" }
    fn name(&self) -> &str { "聚合 HashTable 过大" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGG")
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
                message: format!(
                    "聚合 HashTable 内存使用 {}",
                    format_bytes(memory)
                ),
                suggestions: vec![
                    "检查 GROUP BY 基数是否过高".to_string(),
                    "考虑使用物化视图预聚合".to_string(),
                    "启用 Spill 功能避免 OOM".to_string(),
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
                        name: "streaming_preaggregation_mode".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "auto".to_string(),
                        command: "SET streaming_preaggregation_mode = 'auto';".to_string(),
                    },
                ],
            })
        } else {
            None
        }
    }
}

/// A004: High cardinality GROUP BY
/// Condition: HashTableSize > 10M
pub struct A004HighCardinality;

impl DiagnosticRule for A004HighCardinality {
    fn id(&self) -> &str { "A004" }
    fn name(&self) -> &str { "高基数 GROUP BY" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGG")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let hash_size = context.get_metric("HashTableSize")
            .or_else(|| context.node.metrics.pull_row_num.map(|v| v as f64))?;
        
        if hash_size > 10_000_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "GROUP BY 基数过高 ({:.0} 个分组)",
                    hash_size
                ),
                suggestions: vec![
                    "检查 GROUP BY 键的选择是否合理".to_string(),
                    "考虑使用流式聚合".to_string(),
                    "考虑创建物化视图预聚合".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "enable_sort_aggregate".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "true".to_string(),
                        command: "SET enable_sort_aggregate = true;".to_string(),
                    },
                ],
            })
        } else {
            None
        }
    }
}

/// A003: Aggregation data skew
pub struct A003DataSkew;

impl DiagnosticRule for A003DataSkew {
    fn id(&self) -> &str { "A003" }
    fn name(&self) -> &str { "聚合数据倾斜" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGGREGATE")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let max_input = context.get_metric("__MAX_OF_InputRowCount")?;
        let min_input = context.get_metric("__MIN_OF_InputRowCount").unwrap_or(0.0);
        if min_input == 0.0 { return None; }
        let ratio = max_input / ((max_input + min_input) / 2.0);
        if ratio > 2.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("聚合存在数据倾斜，max/avg 比率为 {:.2}", ratio),
                suggestions: vec!["优化分组键选择".to_string(), "考虑对热点键单独处理".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// A005: Expensive key expression
pub struct A005ExpensiveKeyExpr;

impl DiagnosticRule for A005ExpensiveKeyExpr {
    fn id(&self) -> &str { "A005" }
    fn name(&self) -> &str { "GROUP BY 键表达式计算开销高" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGGREGATE")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let expr_time = context.get_metric("ExprComputeTime")?;
        let agg_time = context.get_metric("AggFuncComputeTime").unwrap_or(1.0);
        if agg_time == 0.0 { return None; }
        let ratio = expr_time / agg_time;
        if ratio > 0.5 && expr_time > 100_000_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("GROUP BY 键表达式计算占比过高 ({:.1}%)", ratio * 100.0),
                suggestions: vec![
                    "在子查询中物化复杂表达式".to_string(),
                    "将表达式提升为生成列".to_string(),
                    "避免在 GROUP BY 中使用复杂函数".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// Get all aggregate rules
pub fn get_rules() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(A001AggregationSkew),
        Box::new(A002HashTableTooLarge),
        Box::new(A003DataSkew),
        Box::new(A004HighCardinality),
        Box::new(A005ExpensiveKeyExpr),
    ]
}
