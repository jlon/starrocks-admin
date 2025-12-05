//! Execution tree builder for DAG visualization
//!
//! Builds the execution tree from topology and fragment data,
//! calculates time percentages and identifies hotspots.

use crate::services::profile_analyzer::models::{
    ExecutionTree, ExecutionTreeNode, Fragment, ProfileSummary, TopologyGraph,
    constants::time_thresholds,
};
use crate::services::profile_analyzer::parser::core::ValueParser;
use crate::services::profile_analyzer::parser::error::{ParseError, ParseResult};
use std::collections::{HashMap, HashSet, VecDeque};

/// Builder for execution tree structure
pub struct TreeBuilder;

impl TreeBuilder {
    /// Build execution tree from topology and nodes
    pub fn build_from_topology(
        topology: &TopologyGraph,
        mut nodes: Vec<ExecutionTreeNode>,
        fragments: &[Fragment],
        summary: &ProfileSummary,
    ) -> ParseResult<ExecutionTree> {
        // Build plan_id to index mapping
        let mut id_to_idx: HashMap<i32, usize> = HashMap::new();
        for (idx, node) in nodes.iter().enumerate() {
            if let Some(plan_id) = node.plan_node_id {
                id_to_idx.insert(plan_id, idx);
            }
        }

        // Set up parent-child relationships from topology
        for topo_node in &topology.nodes {
            if let Some(&node_idx) = id_to_idx.get(&topo_node.id) {
                nodes[node_idx].children.clear();

                for &child_id in &topo_node.children {
                    if let Some(&child_idx) = id_to_idx.get(&child_id) {
                        let child_node_id = nodes[child_idx].id.clone();
                        nodes[node_idx].children.push(child_node_id);
                        nodes[child_idx].parent_plan_node_id = Some(topo_node.id);
                    }
                }
            }
        }

        // Find sink node to use as tree root
        let sink_node_name = Self::find_sink_node_for_tree_root(fragments);

        let root_idx = if let Some(sink_name) = sink_node_name {
            let sink_idx = nodes
                .iter()
                .position(|n| n.operator_name == sink_name)
                .or_else(|| {
                    nodes
                        .iter()
                        .position(|n| n.operator_name.ends_with("_SINK"))
                });

            if let Some(sink_idx) = sink_idx {
                // Connect sink to topology root
                if let Some(&topo_root_idx) = id_to_idx.get(&topology.root_id) {
                    let topo_root_id = nodes[topo_root_idx].id.clone();

                    if !nodes[sink_idx].children.contains(&topo_root_id) {
                        nodes[sink_idx].children.push(topo_root_id);
                    }
                    nodes[topo_root_idx].parent_plan_node_id = nodes[sink_idx].plan_node_id;
                }

                sink_idx
            } else {
                // Bug fix: If sink name is found but sink node doesn't exist in nodes,
                // fall back to topology root with proper error handling
                id_to_idx.get(&topology.root_id).copied().ok_or_else(|| {
                    ParseError::TreeError(format!(
                        "Sink node '{}' not found in nodes and topology root {} not found",
                        sink_name, topology.root_id
                    ))
                })?
            }
        } else {
            id_to_idx.get(&topology.root_id).copied().ok_or_else(|| {
                ParseError::TreeError(format!("Root node {} not found", topology.root_id))
            })?
        };

        // Calculate depths from root
        Self::calculate_depths_from_root(&mut nodes, root_idx)?;

        // Calculate time percentages
        Self::calculate_time_percentages(&mut nodes, summary, fragments)?;

        let root = nodes[root_idx].clone();

        Ok(ExecutionTree { root, nodes })
    }

    /// Build execution tree from fragments only (fallback when no topology)
    pub fn build_from_fragments(
        nodes: Vec<ExecutionTreeNode>,
        summary: &ProfileSummary,
        fragments: &[Fragment],
    ) -> ParseResult<ExecutionTree> {
        if nodes.is_empty() {
            return Err(ParseError::TreeError("No nodes to build tree".to_string()));
        }

        let mut nodes = nodes;

        // Simple linear chain for fallback
        for i in 0..nodes.len().saturating_sub(1) {
            let next_id = nodes[i + 1].id.clone();
            nodes[i].children.push(next_id);
            nodes[i + 1].parent_plan_node_id = nodes[i].plan_node_id;
        }

        Self::calculate_depths(&mut nodes)?;
        Self::calculate_time_percentages(&mut nodes, summary, fragments)?;

        let root = nodes[0].clone();
        Ok(ExecutionTree { root, nodes })
    }

    /// Find sink node to use as tree root
    fn find_sink_node_for_tree_root(fragments: &[Fragment]) -> Option<String> {
        let mut sink_candidates = Vec::new();

        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    let pure_name = Self::extract_operator_name(&operator.name);
                    if pure_name.ends_with("_SINK") {
                        let priority = Self::get_sink_priority(&pure_name);
                        sink_candidates.push((pure_name, priority));
                    }
                }
            }
        }

        sink_candidates.sort_by_key(|(_, priority)| *priority);
        sink_candidates.first().map(|(name, _)| name.clone())
    }

    /// Get priority for sink selection (lower is better)
    fn get_sink_priority(sink_name: &str) -> i32 {
        match sink_name {
            "RESULT_SINK" => 1,
            "OLAP_TABLE_SINK" => 2,
            name if name.contains("TABLE_SINK") => 3,
            name if name.contains("EXCHANGE_SINK") => 10,
            _ => 5,
        }
    }

    /// Extract operator name without plan_node_id suffix
    fn extract_operator_name(full_name: &str) -> String {
        if let Some(pos) = full_name.find(" (plan_node_id=") {
            full_name[..pos].to_string()
        } else {
            full_name.to_string()
        }
    }

    /// Calculate depths from a specific root index
    fn calculate_depths_from_root(
        nodes: &mut [ExecutionTreeNode],
        root_idx: usize,
    ) -> ParseResult<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        let id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (node.id.clone(), idx))
            .collect();

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((root_idx, 0));
        visited.insert(root_idx);
        nodes[root_idx].depth = 0;

        while let Some((node_idx, depth)) = queue.pop_front() {
            let children_ids: Vec<String> = nodes[node_idx].children.clone();

            for child_id in children_ids {
                if let Some(&child_idx) = id_to_idx.get(&child_id)
                    && !visited.contains(&child_idx)
                {
                    nodes[child_idx].depth = depth + 1;
                    visited.insert(child_idx);
                    queue.push_back((child_idx, depth + 1));
                }
            }
        }

        Ok(())
    }

    /// Calculate depths using BFS from detected root
    pub fn calculate_depths(nodes: &mut [ExecutionTreeNode]) -> ParseResult<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        // Find root (node with no parent)
        let mut has_parent = HashSet::new();
        for node in nodes.iter() {
            for child_id in &node.children {
                has_parent.insert(child_id.clone());
            }
        }

        let root_idx = nodes
            .iter()
            .position(|n| !has_parent.contains(&n.id))
            .ok_or_else(|| ParseError::TreeError("No root node found".to_string()))?;

        Self::calculate_depths_from_root(nodes, root_idx)
    }

    /// Calculate time percentages for all nodes
    ///
    /// Following StarRocks' ExplainAnalyzer.computeTimeUsage() logic:
    /// - cpuTime = SUM of all OperatorTotalTime for operators with same plan_node_id
    /// - For ExchangeNode: totalTime = cpuTime + NetworkTime
    /// - For ScanNode: totalTime = cpuTime + ScanTime
    /// - percentage = totalTime * 100.0 / cumulativeOperatorTime
    ///
    /// Reference: ExplainAnalyzer.java#computeTimeUsage()
    pub fn calculate_time_percentages(
        nodes: &mut [ExecutionTreeNode],
        summary: &ProfileSummary,
        fragments: &[Fragment],
    ) -> ParseResult<()> {
        // Determine base time for percentage calculation (QueryCumulativeOperatorTime)
        let base_time_ns = Self::determine_base_time(summary, nodes, fragments);

        if base_time_ns == 0 {
            return Ok(());
        }

        // First, aggregate OperatorTotalTime by plan_node_id from all operators in fragments
        // This follows StarRocks' sumUpMetric() which sums all operator times for same plan_node_id
        let aggregated_times = Self::aggregate_operator_times_by_plan_node_id(fragments);

        // Aggregate NetworkTime by plan_node_id (for EXCHANGE nodes)
        let aggregated_network_times =
            Self::aggregate_metric_by_plan_node_id(fragments, "NetworkTime");

        // Aggregate ScanTime by plan_node_id (for SCAN nodes)
        let aggregated_scan_times = Self::aggregate_metric_by_plan_node_id(fragments, "ScanTime");

        // Calculate percentage for each node following StarRocks logic
        for node in nodes.iter_mut() {
            let plan_id = node.plan_node_id.unwrap_or(-999);

            // Get aggregated CPU time (sum of all OperatorTotalTime for this plan_node_id)
            let cpu_time_ns = aggregated_times.get(&plan_id).copied().unwrap_or(0);
            let mut total_time_ns = cpu_time_ns;

            // For Exchange nodes, add NetworkTime
            // Reference: ExplainAnalyzer.java line 1537-1541
            if node.operator_name.contains("EXCHANGE") {
                // Add aggregated NetworkTime from all fragments
                if let Some(&network_time) = aggregated_network_times.get(&plan_id) {
                    total_time_ns += network_time;
                }
            }

            // For Scan nodes, add ScanTime
            // Reference: ExplainAnalyzer.java line 1542-1547
            if node.operator_name.contains("SCAN") {
                // Add aggregated ScanTime from all fragments
                if let Some(&scan_time) = aggregated_scan_times.get(&plan_id) {
                    total_time_ns += scan_time;
                }
            }

            // Update node's operator_total_time with TOTAL time (cpuTime + extra time)
            // This is what should be displayed as the node's execution time
            if total_time_ns > 0 {
                node.metrics.operator_total_time = Some(total_time_ns);
            }

            if total_time_ns > 0 && base_time_ns > 0 {
                let percentage = (total_time_ns as f64 / base_time_ns as f64) * 100.0;
                // Ensure percentage is valid (not NaN or Infinity)
                if percentage.is_finite() {
                    // Round to 2 decimal places
                    node.time_percentage = Some((percentage * 100.0).round() / 100.0);

                    // Mark consuming nodes (StarRocks thresholds: >30% most, >15% second)
                    // Reference: ExplainAnalyzer.java line 1549-1553
                    if percentage > time_thresholds::MOST_CONSUMING_THRESHOLD {
                        node.is_most_consuming = true;
                        node.is_second_most_consuming = false;
                    } else if percentage > time_thresholds::SECOND_CONSUMING_THRESHOLD {
                        node.is_most_consuming = false;
                        node.is_second_most_consuming = true;
                    }
                }
            }
        }

        Ok(())
    }

    /// Aggregate a specific metric by plan_node_id from all operators in fragments
    /// Searches for __MAX_OF_{metric} first, then falls back to {metric}
    fn aggregate_metric_by_plan_node_id(
        fragments: &[Fragment],
        metric_name: &str,
    ) -> HashMap<i32, u64> {
        let mut aggregated: HashMap<i32, u64> = HashMap::new();
        let max_metric_name = format!("__MAX_OF_{}", metric_name);

        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    let plan_id = operator
                        .plan_node_id
                        .as_ref()
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(-999);

                    // Try __MAX_OF_ first, then fallback to regular metric
                    let time_str = operator
                        .unique_metrics
                        .get(&max_metric_name)
                        .or_else(|| operator.unique_metrics.get(metric_name));

                    if let Some(time_str) = time_str
                        && let Ok(duration) = ValueParser::parse_duration(time_str)
                    {
                        let time_ns = duration.as_nanos() as u64;
                        *aggregated.entry(plan_id).or_insert(0) += time_ns;
                    }
                }
            }
        }

        aggregated
    }

    /// Aggregate OperatorTotalTime by plan_node_id from all operators in fragments
    ///
    /// This follows StarRocks' sumUpMetric() logic which sums all operator times
    /// for operators with the same plan_node_id.
    ///
    /// IMPORTANT: StarRocks uses useMaxValue=true, which means it uses
    /// __MAX_OF_OperatorTotalTime instead of OperatorTotalTime when available.
    ///
    /// Reference: ExplainAnalyzer.java#sumUpMetric() and RuntimeProfile.java#getMaxCounter()
    fn aggregate_operator_times_by_plan_node_id(fragments: &[Fragment]) -> HashMap<i32, u64> {
        let mut aggregated: HashMap<i32, u64> = HashMap::new();

        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    // Parse plan_node_id
                    let plan_id = operator
                        .plan_node_id
                        .as_ref()
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(-999);

                    // Following StarRocks getMaxCounter() logic:
                    // First try __MAX_OF_OperatorTotalTime, then fall back to OperatorTotalTime
                    let time_str = operator
                        .common_metrics
                        .get("__MAX_OF_OperatorTotalTime")
                        .or_else(|| operator.common_metrics.get("OperatorTotalTime"));

                    if let Some(time_str) = time_str
                        && let Ok(duration) = ValueParser::parse_duration(time_str)
                    {
                        let time_ns = duration.as_nanos() as u64;
                        *aggregated.entry(plan_id).or_insert(0) += time_ns;
                    }
                }
            }
        }

        aggregated
    }

    /// Determine base time for percentage calculation
    /// Returns 0 if no valid time can be determined (caller should handle this case)
    fn determine_base_time(
        summary: &ProfileSummary,
        nodes: &[ExecutionTreeNode],
        fragments: &[Fragment],
    ) -> u64 {
        // Try QueryCumulativeOperatorTime first
        if let Some(time_ms) = summary.query_cumulative_operator_time_ms
            && time_ms > 0.0
        {
            return (time_ms * 1_000_000.0) as u64;
        }

        // Try QueryExecutionWallTime
        if let Some(time_ms) = summary.query_execution_wall_time_ms
            && time_ms > 0.0
        {
            return (time_ms * 1_000_000.0) as u64;
        }

        // Fallback: sum all operator times from nodes
        let node_total: u64 = nodes
            .iter()
            .filter_map(|n| n.metrics.operator_total_time)
            .sum();
        if node_total > 0 {
            return node_total;
        }

        // Last resort: sum from fragments
        let fragment_total: u64 = fragments
            .iter()
            .flat_map(|f| &f.pipelines)
            .flat_map(|p| &p.operators)
            .filter_map(|op| op.common_metrics.get("OperatorTotalTime"))
            .filter_map(|s| ValueParser::parse_time_to_ms(s).ok())
            .map(|ms| (ms * 1_000_000.0) as u64)
            .sum();

        fragment_total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_sink_node() {
        let fragments = vec![Fragment {
            id: "0".to_string(),
            backend_addresses: vec![],
            instance_ids: vec![],
            pipelines: vec![crate::services::profile_analyzer::models::Pipeline {
                id: "0".to_string(),
                metrics: HashMap::new(),
                operators: vec![crate::services::profile_analyzer::models::Operator {
                    name: "RESULT_SINK".to_string(),
                    plan_node_id: Some("-1".to_string()),
                    operator_id: None,
                    common_metrics: HashMap::new(),
                    unique_metrics: HashMap::new(),
                    children: vec![],
                }],
            }],
        }];

        let sink = TreeBuilder::find_sink_node_for_tree_root(&fragments);
        assert_eq!(sink, Some("RESULT_SINK".to_string()));
    }

    #[test]
    fn test_get_sink_priority() {
        assert!(
            TreeBuilder::get_sink_priority("RESULT_SINK")
                < TreeBuilder::get_sink_priority("OLAP_TABLE_SINK")
        );
        assert!(
            TreeBuilder::get_sink_priority("OLAP_TABLE_SINK")
                < TreeBuilder::get_sink_priority("EXCHANGE_SINK")
        );
    }
}
