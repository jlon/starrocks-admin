//! Hotspot detection for profile analysis
//! 
//! Identifies performance bottlenecks and issues in query execution.

use crate::services::profile_analyzer::models::*;

/// Hotspot detector for identifying performance issues
pub struct HotSpotDetector;

impl HotSpotDetector {
    /// Analyze a profile and detect hotspots
    pub fn analyze(profile: &Profile) -> Vec<HotSpot> {
        let mut hotspots = Vec::new();
        
        // Check overall query time
        if let Ok(total_time_seconds) = Self::parse_duration(&profile.summary.total_time) {
            if total_time_seconds > 3600.0 {
                hotspots.push(HotSpot {
                    node_path: "Query".to_string(),
                    severity: HotSeverity::Severe,
                    issue_type: "LongRunning".to_string(),
                    description: format!("查询总执行时间过长: {:.1}s", total_time_seconds),
                    suggestions: vec![
                        "检查是否存在数据倾斜".to_string(),
                        "考虑优化查询计划".to_string(),
                        "查看是否存在硬件瓶颈".to_string(),
                    ],
                });
            }
        }
        
        // Analyze execution tree nodes
        if let Some(execution_tree) = &profile.execution_tree {
            for node in &execution_tree.nodes {
                hotspots.extend(Self::analyze_execution_tree_node(node));
            }
        } else {
            // Fallback to fragment analysis
            for fragment in &profile.fragments {
                hotspots.extend(Self::analyze_fragment(fragment));
            }
        }
        
        // Sort by severity (most severe first)
        hotspots.sort_by(|a, b| {
            let severity_order = |severity: &HotSeverity| match severity {
                HotSeverity::Normal => 0,
                HotSeverity::Mild => 1,
                HotSeverity::Moderate => 2,
                HotSeverity::High => 3,
                HotSeverity::Severe => 4,
                HotSeverity::Critical => 5,
            };
            severity_order(&b.severity).cmp(&severity_order(&a.severity))
        });
        
        hotspots
    }
    
    /// Analyze an execution tree node
    fn analyze_execution_tree_node(node: &ExecutionTreeNode) -> Vec<HotSpot> {
        let mut hotspots = Vec::new();
        let node_path = format!("{} (plan_node_id={})", 
            node.operator_name, 
            node.plan_node_id.unwrap_or(-1)
        );
        
        // Check time percentage
        if let Some(percentage) = node.time_percentage {
            if percentage > 50.0 {
                hotspots.push(HotSpot {
                    node_path: node_path.clone(),
                    severity: HotSeverity::Severe,
                    issue_type: "HighTimeCost".to_string(),
                    description: format!("算子 {} 占用 {:.1}% 的执行时间", node.operator_name, percentage),
                    suggestions: Self::get_operator_suggestions(&node.operator_name),
                });
            } else if percentage > 30.0 {
                hotspots.push(HotSpot {
                    node_path: node_path.clone(),
                    severity: HotSeverity::Moderate,
                    issue_type: "HighTimeCost".to_string(),
                    description: format!("算子 {} 占用 {:.1}% 的执行时间", node.operator_name, percentage),
                    suggestions: Self::get_operator_suggestions(&node.operator_name),
                });
            }
        }
        
        // Check memory usage
        if let Some(memory) = node.metrics.memory_usage {
            if memory > 1024 * 1024 * 1024 { // > 1GB
                hotspots.push(HotSpot {
                    node_path: node_path.clone(),
                    severity: HotSeverity::Moderate,
                    issue_type: "HighMemoryUsage".to_string(),
                    description: format!("算子 {} 内存使用过高: {}", node.operator_name, Self::format_bytes(memory)),
                    suggestions: vec![
                        "检查是否内存泄漏".to_string(),
                        "考虑调整内存配置参数".to_string(),
                        "优化数据结构使用".to_string(),
                    ],
                });
            }
        }
        
        hotspots
    }
    
    /// Analyze a fragment
    fn analyze_fragment(fragment: &Fragment) -> Vec<HotSpot> {
        let mut hotspots = Vec::new();
        
        for pipeline in &fragment.pipelines {
            for operator in &pipeline.operators {
                hotspots.extend(Self::analyze_operator(&fragment.id, &pipeline.id, operator));
            }
        }
        
        hotspots
    }
    
    /// Analyze an operator
    fn analyze_operator(fragment_id: &str, pipeline_id: &str, operator: &Operator) -> Vec<HotSpot> {
        let mut hotspots = Vec::new();
        let node_path = format!("Fragment{}.Pipeline{}.{}", fragment_id, pipeline_id, operator.name);
        
        // Check operator time
        if let Some(time_str) = operator.common_metrics.get("OperatorTotalTime") {
            if let Ok(time_seconds) = Self::parse_duration(time_str) {
                if time_seconds > 300.0 {
                    hotspots.push(HotSpot {
                        node_path: node_path.clone(),
                        severity: HotSeverity::Severe,
                        issue_type: "HighTimeCost".to_string(),
                        description: format!("算子 {} 耗时过高: {:.1}s", operator.name, time_seconds),
                        suggestions: Self::get_operator_suggestions(&operator.name),
                    });
                }
            }
        }
        
        hotspots
    }
    
    /// Get suggestions for specific operator types
    fn get_operator_suggestions(operator_name: &str) -> Vec<String> {
        match operator_name.to_uppercase().as_str() {
            name if name.contains("SCAN") => vec![
                "检查是否可以添加过滤条件减少扫描数据量".to_string(),
                "考虑添加索引或物化视图".to_string(),
                "检查分区裁剪是否生效".to_string(),
            ],
            name if name.contains("JOIN") => vec![
                "检查JOIN顺序是否最优".to_string(),
                "考虑使用Runtime Filter".to_string(),
                "检查是否存在数据倾斜".to_string(),
            ],
            name if name.contains("AGGREGATE") => vec![
                "检查聚合模式是否合适".to_string(),
                "考虑使用预聚合或物化视图".to_string(),
                "检查GROUP BY键的选择".to_string(),
            ],
            name if name.contains("EXCHANGE") => vec![
                "检查数据分布是否均匀".to_string(),
                "考虑调整并行度".to_string(),
                "检查网络带宽是否充足".to_string(),
            ],
            _ => vec![
                "检查该算子是否处理数据量过大".to_string(),
                "考虑优化查询计划".to_string(),
            ],
        }
    }
    
    /// Parse duration string to seconds
    fn parse_duration(duration_str: &str) -> Result<f64, ()> {
        let duration_str = duration_str.trim();
        
        if duration_str.contains("h") {
            let parts: Vec<&str> = duration_str.split('h').collect();
            let hours: f64 = parts.first().unwrap_or(&"0").parse().unwrap_or(0.0);
            let rest = parts.get(1).unwrap_or(&"0");
            let minutes: f64 = rest.split('m').next().unwrap_or("0").parse().unwrap_or(0.0);
            Ok(hours * 3600.0 + minutes * 60.0)
        } else if duration_str.contains("m") && !duration_str.contains("ms") {
            let minutes: f64 = duration_str.split('m').next().unwrap_or("0").parse().unwrap_or(0.0);
            Ok(minutes * 60.0)
        } else if duration_str.ends_with("ms") {
            let ms: f64 = duration_str.trim_end_matches("ms").parse().unwrap_or(0.0);
            Ok(ms / 1000.0)
        } else if duration_str.ends_with("us") || duration_str.ends_with("μs") {
            let us: f64 = duration_str.trim_end_matches("us").trim_end_matches("μs").parse().unwrap_or(0.0);
            Ok(us / 1_000_000.0)
        } else if duration_str.ends_with("ns") {
            let ns: f64 = duration_str.trim_end_matches("ns").parse().unwrap_or(0.0);
            Ok(ns / 1_000_000_000.0)
        } else if duration_str.ends_with("s") {
            let s: f64 = duration_str.trim_end_matches("s").parse().unwrap_or(0.0);
            Ok(s)
        } else {
            Err(())
        }
    }
    
    /// Format bytes to human-readable string
    fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}
