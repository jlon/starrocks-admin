//! Aggregate operator diagnostic rules (A001-A005)
//!
//! Rules for AGGREGATE operators.

use super::*;

/// A001: Aggregation skew
/// Condition: max(AggComputeTime)/avg > threshold (dynamic based on cluster size)
/// P0.2: Added absolute value protection (min 100k rows aggregated)
/// v2.0: Uses dynamic skew threshold based on cluster parallelism
pub struct A001AggregationSkew;

impl DiagnosticRule for A001AggregationSkew {
    fn id(&self) -> &str {
        "A001"
    }
    fn name(&self) -> &str {
        "聚合数据倾斜"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGG")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let max_time = context.node.metrics.operator_total_time_max?;
        let avg_time = context.node.metrics.operator_total_time?;

        if avg_time == 0 {
            return None;
        }

        // P0.2: Absolute value protection - only check if aggregation is significant
        // v2.0: Use dynamic threshold from thresholds module
        let min_rows_threshold = context.thresholds.get_min_rows_for_skew();
        let input_rows = context.get_metric("PushRowNum").unwrap_or(0.0);

        if input_rows < min_rows_threshold {
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
                node_path: format!("{} (plan_node_id={})",
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "聚合存在数据倾斜，max/avg 比率为 {:.2} (阈值: {:.1})",
                    ratio, skew_threshold
                ),
                reason: "聚合算子多个实例处理的数据量存在明显差异，部分实例成为瓶颈。通常是 GROUP BY 键的数据分布不均匀导致。".to_string(),
                suggestions: vec![
                    "检查 GROUP BY 键的数据分布".to_string(),
                    "考虑使用两阶段聚合".to_string(),
                    "检查是否存在热点键".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// A002: HashTable memory too large
/// Condition: HashTableMemoryUsage > threshold (dynamic based on BE memory)
/// v2.0: Uses dynamic hash table memory threshold (5% of BE memory, clamped to 512MB-5GB)
pub struct A002HashTableTooLarge;

impl DiagnosticRule for A002HashTableTooLarge {
    fn id(&self) -> &str {
        "A002"
    }
    fn name(&self) -> &str {
        "聚合 HashTable 过大"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGG")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let memory = context.get_memory_usage()?;

        // v2.0: Use dynamic hash table memory threshold
        let memory_threshold = context.thresholds.get_hash_table_memory_threshold();

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
                    "聚合 HashTable 内存使用 {} (阈值: {})",
                    format_bytes(memory),
                    format_bytes(memory_threshold)
                ),
                reason: "HashTable 占用内存过大，可能导致内存压力或触发 Spill。通常是 GROUP BY 键基数过高或聚合函数状态过大。".to_string(),
                suggestions: vec![
                    "检查 GROUP BY 基数是否过高".to_string(),
                    "考虑使用物化视图预聚合".to_string(),
                    "启用 Spill 功能避免 OOM".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    if let Some(s) = context.suggest_parameter_smart("enable_spill") {
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

/// A004: High cardinality GROUP BY
/// Condition: HashTableSize > 10M
pub struct A004HighCardinality;

impl DiagnosticRule for A004HighCardinality {
    fn id(&self) -> &str {
        "A004"
    }
    fn name(&self) -> &str {
        "高基数 GROUP BY"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGG")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let hash_size = context
            .get_metric("HashTableSize")
            .or_else(|| context.node.metrics.pull_row_num.map(|v| v as f64))?;

        if hash_size > 10_000_000.0 {
            let group_keys = context
                .get_group_by_keys()
                .unwrap_or_else(|| "未知".to_string());
            let memory = context.get_memory_usage().unwrap_or(0);

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
                message: format!("GROUP BY 基数过高 ({:.0} 个分组)", hash_size),
                reason: format!(
                    "GROUP BY 键「{}」的基数过高（{:.0} 个唯一值），导致 HashTable 占用 {} 内存，聚合效率下降。",
                    group_keys,
                    hash_size,
                    format_bytes(memory)
                ),
                suggestions: vec![
                    format!("检查 GROUP BY 键「{}」是否都必要，减少不必要的分组列", group_keys),
                    "考虑使用流式聚合: SET streaming_preaggregation_mode = 'force_streaming'"
                        .to_string(),
                    "考虑创建物化视图预聚合常用分组".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// A003: Aggregation data skew
pub struct A003DataSkew;

impl DiagnosticRule for A003DataSkew {
    fn id(&self) -> &str {
        "A003"
    }
    fn name(&self) -> &str {
        "聚合数据倾斜"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGGREGATE")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let max_input = context.get_metric("__MAX_OF_InputRowCount")?;
        let min_input = context.get_metric("__MIN_OF_InputRowCount").unwrap_or(0.0);
        if min_input == 0.0 {
            return None;
        }
        let ratio = max_input / ((max_input + min_input) / 2.0);
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
                message: format!("聚合存在数据倾斜，max/avg 比率为 {:.2}", ratio),
                reason: "聚合算子的输入数据在各个实例间分布不均匀，导致部分实例处理更多数据。"
                    .to_string(),
                suggestions: vec!["优化分组键选择".to_string(), "考虑对热点键单独处理".to_string()],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// A005: Expensive key expression
pub struct A005ExpensiveKeyExpr;

impl DiagnosticRule for A005ExpensiveKeyExpr {
    fn id(&self) -> &str {
        "A005"
    }
    fn name(&self) -> &str {
        "GROUP BY 键表达式计算开销高"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("AGGREGATE")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let expr_time = context.get_metric("ExprComputeTime")?;
        let agg_time = context.get_metric("AggFuncComputeTime").unwrap_or(1.0);
        if agg_time == 0.0 {
            return None;
        }
        let ratio = expr_time / agg_time;
        if ratio > 0.5 && expr_time > 100_000_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!("GROUP BY 键表达式计算占比过高 ({:.1}%)", ratio * 100.0),
                reason: "GROUP BY 键包含复杂表达式，每行数据都需要计算表达式，增加 CPU 开销。建议将表达式提前计算或使用生成列。".to_string(),
                suggestions: vec![
                    "在子查询中物化复杂表达式".to_string(),
                    "将表达式提升为生成列".to_string(),
                    "避免在 GROUP BY 中使用复杂函数".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// A006: Low local aggregation efficiency
/// Condition: InputRowCount / OutputRowCount < 2.0 (aggregation reduces less than 50%)
pub struct A006LowLocalAggregation;

impl DiagnosticRule for A006LowLocalAggregation {
    fn id(&self) -> &str {
        "A006"
    }
    fn name(&self) -> &str {
        "Aggregate 本地聚合度低"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        let name = node.operator_name.to_uppercase();
        // Only apply to local/first-stage aggregation
        name.contains("AGGREGATE") || name.contains("AGG")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        // Get input and output row counts
        let input_rows = context
            .get_metric("InputRowCount")
            .or_else(|| context.node.metrics.pull_row_num.map(|v| v as f64))?;
        let output_rows = context
            .get_metric("OutputRowCount")
            .or_else(|| context.node.metrics.push_row_num.map(|v| v as f64))?;

        if output_rows == 0.0 || input_rows == 0.0 {
            return None;
        }

        // Calculate aggregation ratio
        let agg_ratio = input_rows / output_rows;

        // If aggregation ratio < 2.0 (less than 50% reduction), local aggregation is ineffective
        // Also check that we have significant data (> 10K rows)
        if agg_ratio < 2.0 && input_rows > 10_000.0 {
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
                    "本地聚合效果差，聚合比 {:.2}:1 (输入 {:.0} 行 → 输出 {:.0} 行)",
                    agg_ratio, input_rows, output_rows
                ),
                reason: {
                    let group_keys = context
                        .get_group_by_keys()
                        .unwrap_or_else(|| "未知".to_string());
                    format!(
                        "GROUP BY「{}」在本地聚合时，输入 {:.0} 行仅聚合为 {:.0} 行（缩减比 {:.2}:1），\
                        未能有效减少数据量。这会增加网络传输和后续计算开销。",
                        group_keys, input_rows, output_rows, agg_ratio
                    )
                },
                suggestions: {
                    let group_keys = context
                        .get_group_by_keys()
                        .unwrap_or_else(|| "未知".to_string());
                    vec![
                        format!(
                            "GROUP BY「{}」基数可能过高，考虑关闭二阶段聚合: SET new_planner_agg_stage = 1",
                            group_keys
                        ),
                        "检查 GROUP BY 键是否包含高基数列（如 ID、时间戳）".to_string(),
                        "考虑在数据写入时预聚合或使用物化视图".to_string(),
                    ]
                },
                parameter_suggestions: vec![ParameterSuggestion::new(
                    "new_planner_agg_stage",
                    ParameterType::Session,
                    None,
                    "1",
                    "SET new_planner_agg_stage = 1; -- 关闭二阶段聚合",
                )],
                threshold_metadata: None,
            })
        } else {
            None
        }
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
        Box::new(A006LowLocalAggregation),
    ]
}
