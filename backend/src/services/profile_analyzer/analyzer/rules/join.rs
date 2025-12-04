//! Join operator diagnostic rules (J001-J010)
//!
//! Rules for HASH_JOIN, CROSS_JOIN, NEST_LOOP_JOIN operators.

use super::*;

/// J001: Join result explosion
/// Condition: PullRowNum > ProbeRows * 10
pub struct J001ResultExplosion;

impl DiagnosticRule for J001ResultExplosion {
    fn id(&self) -> &str { "J001" }
    fn name(&self) -> &str { "Join 结果膨胀" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let output_rows = context.node.metrics.pull_row_num? as f64;
        let probe_rows = context.get_metric("ProbeRows")?;
        
        if probe_rows == 0.0 {
            return None;
        }
        
        let ratio = output_rows / probe_rows;
        
        if ratio > 10.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Error,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "Join 结果膨胀 {:.1} 倍 (输出 {:.0} 行 / 探测 {:.0} 行)",
                    ratio, output_rows, probe_rows
                ),
                suggestions: vec![
                    "检查 JOIN 条件是否缺失或不完整".to_string(),
                    "检查是否存在多对多关系".to_string(),
                    "考虑添加更多过滤条件".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// J002: Build side larger than probe side
/// Condition: BuildRows > ProbeRows
pub struct J002BuildLargerThanProbe;

impl DiagnosticRule for J002BuildLargerThanProbe {
    fn id(&self) -> &str { "J002" }
    fn name(&self) -> &str { "Join Build 端过大" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("HASH") && 
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let build_rows = context.get_metric("BuildRows")?;
        let probe_rows = context.get_metric("ProbeRows")?;
        
        if build_rows > probe_rows && build_rows > 100_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "Build 端行数 ({:.0}) 大于 Probe 端 ({:.0})，Join 顺序可能不优",
                    build_rows, probe_rows
                ),
                suggestions: vec![
                    "执行 ANALYZE TABLE 更新统计信息".to_string(),
                    "检查优化器是否选择了正确的 Join 顺序".to_string(),
                    "考虑使用 Hint 指定 Join 顺序".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// J003: HashTable memory too large
/// Condition: HashTableMemoryUsage > 1GB
pub struct J003HashTableTooLarge;

impl DiagnosticRule for J003HashTableTooLarge {
    fn id(&self) -> &str { "J003" }
    fn name(&self) -> &str { "HashTable 内存过大" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("HASH") && 
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let hash_memory = context.get_metric("HashTableMemoryUsage")
            .or_else(|| context.get_memory_usage().map(|v| v as f64))?;
        
        const ONE_GB: f64 = 1024.0 * 1024.0 * 1024.0;
        
        if hash_memory > ONE_GB {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "HashTable 内存使用 {}，可能导致内存压力",
                    format_bytes(hash_memory as u64)
                ),
                suggestions: vec![
                    "检查 Build 端数据量是否过大".to_string(),
                    "考虑使用 Runtime Filter 减少数据量".to_string(),
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
                ],
            })
        } else {
            None
        }
    }
}

/// J004: Runtime Filter not generated
/// Condition: RuntimeFilterNum == 0 && BuildRows > 10000
pub struct J004NoRuntimeFilter;

impl DiagnosticRule for J004NoRuntimeFilter {
    fn id(&self) -> &str { "J004" }
    fn name(&self) -> &str { "未生成 Runtime Filter" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let rf_num = context.get_metric("RuntimeFilterNum").unwrap_or(0.0);
        let build_rows = context.get_metric("BuildRows").unwrap_or(0.0);
        
        if rf_num == 0.0 && build_rows > 10_000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "Join 未生成 Runtime Filter，Build 端有 {:.0} 行",
                    build_rows
                ),
                suggestions: vec![
                    "检查 enable_global_runtime_filter 是否启用".to_string(),
                    "检查 Join 条件是否适合生成 RF".to_string(),
                    "检查 Build 端行数是否超过阈值".to_string(),
                ],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "enable_global_runtime_filter".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "true".to_string(),
                        command: "SET enable_global_runtime_filter = true;".to_string(),
                    },
                    ParameterSuggestion {
                        name: "runtime_join_filter_push_down_limit".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "10000000".to_string(),
                        command: "SET runtime_join_filter_push_down_limit = 10000000;".to_string(),
                    },
                ],
            })
        } else {
            None
        }
    }
}

/// J009: Non-equi join fallback
/// Condition: JoinType contains CROSS or NESTLOOP
pub struct J009NonEquiJoin;

impl DiagnosticRule for J009NonEquiJoin {
    fn id(&self) -> &str { "J009" }
    fn name(&self) -> &str { "非等式 Join 回退" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        let name = node.operator_name.to_uppercase();
        name.contains("CROSS") || name.contains("NEST") || name.contains("LOOP")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let probe_rows = context.get_metric("ProbeRows").unwrap_or(0.0);
        let build_rows = context.get_metric("BuildRows").unwrap_or(0.0);
        
        // Only warn if significant data volume
        if probe_rows > 1000.0 || build_rows > 1000.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "使用了 {} 算子，可能是非等式 Join 导致",
                    context.node.operator_name
                ),
                suggestions: vec![
                    "检查 JOIN 条件是否包含等式条件".to_string(),
                    "尝试将非等式条件转换为等式条件".to_string(),
                    "考虑重构查询逻辑".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// J010: Probe cache unfriendly
/// Condition: ProbeRows >> BuildRows && HashTableMemory > L3 Cache
pub struct J010ProbeCacheUnfriendly;

impl DiagnosticRule for J010ProbeCacheUnfriendly {
    fn id(&self) -> &str { "J010" }
    fn name(&self) -> &str { "探测缓存不友好" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("HASH") && 
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let hash_memory = context.get_metric("HashTableMemoryUsage")
            .or_else(|| context.get_memory_usage().map(|v| v as f64))?;
        let probe_rows = context.get_metric("ProbeRows")?;
        let build_rows = context.get_metric("BuildRows")?;
        
        // L3 cache typically 30-50MB, use 50MB as threshold
        const L3_CACHE: f64 = 50.0 * 1024.0 * 1024.0;
        
        if hash_memory > L3_CACHE && probe_rows > build_rows * 100.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", 
                    context.node.operator_name,
                    context.node.plan_node_id.unwrap_or(-1)),
                message: format!(
                    "HashTable ({}) 超过 L3 缓存，探测行数 ({:.0}) 远大于构建行数 ({:.0})，可能存在缓存不友好",
                    format_bytes(hash_memory as u64), probe_rows, build_rows
                ),
                suggestions: vec![
                    "考虑交换 Join 左右表顺序".to_string(),
                    "使用 Hint 指定 Join 顺序".to_string(),
                    "检查统计信息是否准确".to_string(),
                ],
                parameter_suggestions: vec![],
            })
        } else {
            None
        }
    }
}

/// J005: Hash collision severe
pub struct J005HashCollision;

impl DiagnosticRule for J005HashCollision {
    fn id(&self) -> &str { "J005" }
    fn name(&self) -> &str { "Hash 碰撞严重" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let keys_per_bucket = context.get_metric("BuildKeysPerBucket%")?;
        if keys_per_bucket > 10.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("Hash 表碰撞严重，平均每桶 {:.1} 个键", keys_per_bucket),
                suggestions: vec!["优化 Join 键选择".to_string(), "检查数据是否存在大量重复值".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// J006: Join shuffle skew
pub struct J006ShuffleSkew;

impl DiagnosticRule for J006ShuffleSkew {
    fn id(&self) -> &str { "J006" }
    fn name(&self) -> &str { "Join Shuffle 倾斜" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let max_probe = context.get_metric("__MAX_OF_ProbeRows")?;
        let min_probe = context.get_metric("__MIN_OF_ProbeRows").unwrap_or(0.0);
        if min_probe == 0.0 { return None; }
        let ratio = max_probe / ((max_probe + min_probe) / 2.0);
        if ratio > 3.0 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("Join 数据分布倾斜，max/avg 比率为 {:.2}", ratio),
                suggestions: vec!["切换到更高基数的连接键".to_string(), "对键添加盐值".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// J007: Partition join probe overhead high
pub struct J007PartitionProbeOverhead;

impl DiagnosticRule for J007PartitionProbeOverhead {
    fn id(&self) -> &str { "J007" }
    fn name(&self) -> &str { "分区 Join 探测开销高" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let partition_nums = context.get_metric("PartitionNums")?;
        let probe_overhead = context.get_metric("PartitionProbeOverhead").unwrap_or(0.0);
        let search_time = context.get_metric("SearchHashTableTime").unwrap_or(1.0);
        if partition_nums > 1.0 && search_time > 0.0 && probe_overhead / search_time > 0.5 {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Warning,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("分区探测开销占比 {:.1}%，分区数为 {:.0}", probe_overhead / search_time * 100.0, partition_nums),
                suggestions: vec!["检查分区数是否合理".to_string(), "考虑增加内存限制避免过度分区".to_string()],
                parameter_suggestions: vec![],
            })
        } else { None }
    }
}

/// J008: Runtime filter memory high
pub struct J008RFMemoryHigh;

impl DiagnosticRule for J008RFMemoryHigh {
    fn id(&self) -> &str { "J008" }
    fn name(&self) -> &str { "Runtime Filter 内存占用高" }
    
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool {
        node.operator_name.to_uppercase().contains("JOIN")
    }
    
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic> {
        let rf_bytes = context.get_metric("PartialRuntimeMembershipFilterBytes")?;
        const HUNDRED_MB: f64 = 100.0 * 1024.0 * 1024.0;
        if rf_bytes > HUNDRED_MB {
            Some(Diagnostic {
                rule_id: self.id().to_string(),
                rule_name: self.name().to_string(),
                severity: RuleSeverity::Info,
                node_path: format!("{} (plan_node_id={})", context.node.operator_name, context.node.plan_node_id.unwrap_or(-1)),
                message: format!("Runtime Filter 内存占用 {}", format_bytes(rf_bytes as u64)),
                suggestions: vec!["降低 runtime_filter_max_size 配置".to_string(), "检查 Join 键基数是否过高".to_string()],
                parameter_suggestions: vec![
                    ParameterSuggestion {
                        name: "runtime_filter_max_size".to_string(),
                        param_type: ParameterType::Session,
                        current: None,
                        recommended: "67108864".to_string(),
                        command: "SET runtime_filter_max_size = 67108864; -- 64MB".to_string(),
                    },
                ],
            })
        } else { None }
    }
}

/// Get all join rules
pub fn get_rules() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        Box::new(J001ResultExplosion),
        Box::new(J002BuildLargerThanProbe),
        Box::new(J003HashTableTooLarge),
        Box::new(J004NoRuntimeFilter),
        Box::new(J005HashCollision),
        Box::new(J006ShuffleSkew),
        Box::new(J007PartitionProbeOverhead),
        Box::new(J008RFMemoryHigh),
        Box::new(J009NonEquiJoin),
        Box::new(J010ProbeCacheUnfriendly),
    ]
}
