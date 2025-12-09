//! Exchange operator diagnostic rules (E001-E003)
//!
//! Rules for EXCHANGE_SINK and EXCHANGE_SOURCE operators.

use super::*;

/// E001: Network transfer too large
/// Condition: BytesSent > 1GB
pub struct E001NetworkTransferLarge;

impl DiagnosticRule for E001NetworkTransferLarge {
    fn id(&self) -> &str {
        "E001"
    }
    fn name(&self) -> &str {
        "网络传输数据量大"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("EXCHANGE")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let bytes_sent = context
            .get_metric("BytesSent")
            .or_else(|| context.get_metric("NetworkBytesSent"))?;

        const ONE_GB: f64 = 1024.0 * 1024.0 * 1024.0;

        if bytes_sent > ONE_GB {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "网络传输数据量 {}，可能存在网络瓶颈",
                    format_bytes(bytes_sent as u64)
                ),
                reason: "网络传输数据量过大，占用大量网络带宽和时间。可能是 Shuffle 数据量大或缺少有效的数据裁剪。".to_string(),
                suggestions: vec![
                    "检查是否可以减少 Shuffle 数据量".to_string(),
                    "考虑使用 Colocate Join 避免 Shuffle".to_string(),
                    "检查网络带宽是否充足".to_string(),
                ],
                parameter_suggestions: {
                    let mut suggestions = Vec::new();
                    // Use smart recommendation for parallel_fragment_exec_instance_num
                    if let Some(s) = context.suggest_parameter_smart("parallel_fragment_exec_instance_num") {
                        suggestions.push(s);
                    }
                    // Use smart recommendation for pipeline_dop
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

/// E002: Network time too high
/// Condition: NetworkTime/OperatorTime > 0.5
pub struct E002NetworkTimeHigh;

impl DiagnosticRule for E002NetworkTimeHigh {
    fn id(&self) -> &str {
        "E002"
    }
    fn name(&self) -> &str {
        "网络时间占比高"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("EXCHANGE")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let network_time = context
            .get_metric("NetworkTime")
            .or_else(|| context.get_metric("OverallThroughputSendTime"))?;
        let operator_time = context.get_operator_time_ms()?;

        if operator_time == 0.0 {
            return None;
        }

        let ratio = network_time / operator_time;

        if ratio > 0.5 {
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
                message: format!("网络时间占比 {:.1}%，可能存在网络瓶颈", ratio * 100.0),
                reason: "网络传输时间占比过高，查询瓶颈在网络。可能是网络带宽不足或跨机房传输。"
                    .to_string(),
                suggestions: vec![
                    "检查网络带宽和延迟".to_string(),
                    "考虑使用 Colocate Join".to_string(),
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

/// E003: Data skew in shuffle
/// Condition: max(BytesSent)/avg > 2
pub struct E003ShuffleSkew;

impl DiagnosticRule for E003ShuffleSkew {
    fn id(&self) -> &str {
        "E003"
    }
    fn name(&self) -> &str {
        "Shuffle 数据倾斜"
    }

    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("EXCHANGE")
    }

    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let max_bytes = context.get_metric("__MAX_OF_BytesSent")?;
        let min_bytes = context.get_metric("__MIN_OF_BytesSent").unwrap_or(0.0);

        if min_bytes == 0.0 {
            return None;
        }

        let avg_bytes = (max_bytes + min_bytes) / 2.0;
        let ratio = max_bytes / avg_bytes;

        if ratio > 2.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                plan_node_id: context.node.plan_node_id,
                message: format!(
                    "Shuffle 存在数据倾斜，max/avg 比率为 {:.2}",
                    ratio
                ),
                reason: "Shuffle 数据在各节点间分布不均匀，部分节点接收更多数据。通常是 Shuffle 键存在热点值。".to_string(),
                suggestions: vec![
                    "检查分区键选择是否合理".to_string(),
                    "考虑使用 Skew Join 优化".to_string(),
                    "检查是否存在热点数据".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }
}

/// Get all exchange rules
pub fn get_rules() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(E001NetworkTransferLarge),
        Box::new(E002NetworkTimeHigh),
        Box::new(E003ShuffleSkew),
    ]
}
