//! Profile composer - main entry point for profile parsing
//!
//! Orchestrates all parsing components to produce a complete Profile structure.

use crate::services::profile_analyzer::models::{
    ExecutionTreeNode, Fragment, HotSeverity, OperatorMetrics, Profile, ProfileSummary, TopNode,
    TopologyGraph, constants::time_thresholds,
};
use crate::services::profile_analyzer::parser::core::{
    FragmentParser, MetricsParser, OperatorParser, SectionParser, TopologyParser, TreeBuilder,
    ValueParser,
};
use crate::services::profile_analyzer::parser::error::{ParseError, ParseResult};
use crate::services::profile_analyzer::parser::specialized::SpecializedMetricsParser;
use std::collections::HashMap;

/// Main profile composer that orchestrates all parsing
#[derive(Debug, Clone)]
pub struct ProfileComposer {
    specialized_parser: SpecializedMetricsParser,
}

impl Default for ProfileComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileComposer {
    pub fn new() -> Self {
        Self { specialized_parser: SpecializedMetricsParser::new() }
    }

    /// Parse a complete profile from text
    pub fn parse(&mut self, text: &str) -> ParseResult<Profile> {
        // Parse main sections
        let mut summary = SectionParser::parse_summary(text)?;
        let planner_info = SectionParser::parse_planner(text)?;
        let execution_info = SectionParser::parse_execution(text)?;

        // Extract additional metrics from execution info
        if summary.query_cumulative_operator_time_ms.is_none()
            && let Some(qcot) = execution_info.metrics.get("QueryCumulativeOperatorTime")
        {
            summary.query_cumulative_operator_time_ms = ValueParser::parse_time_to_ms(qcot).ok();
            summary.query_cumulative_operator_time = Some(qcot.clone());
        }

        if summary.query_execution_wall_time_ms.is_none()
            && let Some(qewt) = execution_info.metrics.get("QueryExecutionWallTime")
        {
            summary.query_execution_wall_time_ms = ValueParser::parse_time_to_ms(qewt).ok();
            summary.query_execution_wall_time = Some(qewt.clone());
        }

        // Extract all execution metrics
        SectionParser::extract_execution_metrics(&execution_info, &mut summary);

        // Parse fragments
        let fragments = FragmentParser::extract_all_fragments(text);

        // Parse topology and build execution tree
        let topology_result = Self::extract_topology_json(&execution_info.topology)
            .and_then(|json| TopologyParser::parse_with_fragments(&json, text, &fragments))
            .ok();

        let execution_tree = if let Some(ref topology) = topology_result {
            let nodes = self.build_nodes_from_topology_and_fragments(topology, &fragments)?;
            TreeBuilder::build_from_topology(topology, nodes, &fragments, &summary)?
        } else {
            let nodes = self.build_nodes_from_fragments(text, &fragments)?;
            TreeBuilder::build_from_fragments(nodes, &summary, &fragments)?
        };

        // Compute top time-consuming nodes
        let top_nodes = Self::compute_top_time_consuming_nodes(&execution_tree.nodes, 3);
        summary.top_time_consuming_nodes = Some(top_nodes);

        // Analyze profile completeness (check for MissingInstanceIds)
        Self::analyze_profile_completeness(text, &mut summary);

        Ok(Profile {
            summary,
            planner: planner_info,
            execution: execution_info,
            fragments,
            execution_tree: Some(execution_tree),
        })
    }

    /// Extract topology JSON from topology text
    fn extract_topology_json(topology_text: &str) -> ParseResult<String> {
        if topology_text.trim().is_empty() {
            return Err(ParseError::TopologyError("Empty topology text".to_string()));
        }

        if let Some(start) = topology_text.find("Topology: ") {
            let json_start = start + "Topology: ".len();
            let json_part = &topology_text[json_start..];
            let json_end = json_part.find('\n').unwrap_or(json_part.len());
            let json = json_part[..json_end].trim();

            if json.is_empty() {
                return Err(ParseError::TopologyError("Empty JSON after Topology:".to_string()));
            }

            Ok(json.to_string())
        } else {
            Ok(topology_text.trim().to_string())
        }
    }

    /// Build execution tree nodes from topology and fragments
    fn build_nodes_from_topology_and_fragments(
        &self,
        topology: &TopologyGraph,
        fragments: &[Fragment],
    ) -> ParseResult<Vec<ExecutionTreeNode>> {
        // Build operator lookup by plan_node_id
        let mut operators_by_plan_id: HashMap<
            i32,
            Vec<(&crate::services::profile_analyzer::models::Operator, String, String)>,
        > = HashMap::new();

        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    if let Some(plan_id) = &operator.plan_node_id
                        && let Ok(plan_id_int) = plan_id.parse::<i32>()
                    {
                        operators_by_plan_id.entry(plan_id_int).or_default().push((
                            operator,
                            fragment.id.clone(),
                            pipeline.id.clone(),
                        ));
                    }
                }
            }
        }

        let mut nodes = Vec::new();

        // Create nodes from topology
        for topo_node in &topology.nodes {
            let tree_node = if let Some(op_list) = operators_by_plan_id.get(&topo_node.id) {
                let op_refs: Vec<&crate::services::profile_analyzer::models::Operator> =
                    op_list.iter().map(|(op, _, _)| *op).collect();
                let aggregated_op = Self::aggregate_operators(&op_refs, &topo_node.name);

                let (frag_id, pipe_id) = op_list
                    .first()
                    .map(|(_, f, p)| (Some(f.clone()), Some(p.clone())))
                    .unwrap_or((None, None));

                let mut metrics = MetricsParser::from_hashmap(&aggregated_op.common_metrics);

                // Also parse memory-related metrics from unique_metrics
                MetricsParser::merge_memory_metrics(&mut metrics, &aggregated_op.unique_metrics);

                // Parse specialized metrics
                if !aggregated_op.unique_metrics.is_empty() {
                    let pure_name = Self::extract_operator_name(&aggregated_op.name);
                    let operator_text =
                        Self::build_operator_text(&pure_name, topo_node.id, &aggregated_op);
                    metrics.specialized = self.specialized_parser.parse(&pure_name, &operator_text);
                }

                let rows = metrics.push_row_num.or(metrics.pull_row_num);

                ExecutionTreeNode {
                    id: format!("node_{}", topo_node.id),
                    plan_node_id: Some(topo_node.id),
                    operator_name: topo_node.name.clone(),
                    node_type: OperatorParser::determine_node_type(&aggregated_op.name),
                    parent_plan_node_id: None,
                    children: Vec::new(),
                    depth: 0,
                    metrics,
                    is_hotspot: false,
                    hotspot_severity: HotSeverity::Normal,
                    fragment_id: frag_id,
                    pipeline_id: pipe_id,
                    time_percentage: None,
                    rows,
                    is_most_consuming: false,
                    is_second_most_consuming: false,
                    unique_metrics: aggregated_op.unique_metrics.clone(),
                    has_diagnostic: false,
                    diagnostic_ids: Vec::new(),
                }
            } else {
                ExecutionTreeNode {
                    id: format!("node_{}", topo_node.id),
                    plan_node_id: Some(topo_node.id),
                    operator_name: topo_node.name.clone(),
                    node_type: OperatorParser::determine_node_type(&topo_node.name),
                    parent_plan_node_id: None,
                    children: Vec::new(),
                    depth: 0,
                    metrics: OperatorMetrics::default(),
                    is_hotspot: false,
                    hotspot_severity: HotSeverity::Normal,
                    fragment_id: None,
                    pipeline_id: None,
                    time_percentage: None,
                    rows: None,
                    is_most_consuming: false,
                    is_second_most_consuming: false,
                    unique_metrics: HashMap::new(),
                    has_diagnostic: false,
                    diagnostic_ids: Vec::new(),
                }
            };

            nodes.push(tree_node);
        }

        // Add sink nodes not in topology
        self.add_sink_nodes(&mut nodes, fragments, topology);

        Ok(nodes)
    }

    /// Add sink nodes that are not in the topology
    fn add_sink_nodes(
        &self,
        nodes: &mut Vec<ExecutionTreeNode>,
        fragments: &[Fragment],
        topology: &TopologyGraph,
    ) {
        let mut next_sink_id = -1;

        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    let pure_name = Self::extract_operator_name(&operator.name);

                    if pure_name.ends_with("_SINK") {
                        let plan_id = operator
                            .plan_node_id
                            .as_ref()
                            .and_then(|id| id.parse::<i32>().ok())
                            .unwrap_or(next_sink_id);

                        // Check if already in topology
                        if !topology.nodes.iter().any(|n| n.id == plan_id) {
                            let metrics = MetricsParser::from_hashmap(&operator.common_metrics);
                            let rows = metrics.push_row_num.or(metrics.pull_row_num);

                            let sink_node = ExecutionTreeNode {
                                id: format!("sink_{}", plan_id.abs()),
                                plan_node_id: Some(plan_id),
                                operator_name: pure_name.clone(),
                                node_type: OperatorParser::determine_node_type(&pure_name),
                                parent_plan_node_id: None,
                                children: Vec::new(),
                                depth: 0,
                                metrics,
                                is_hotspot: false,
                                hotspot_severity: HotSeverity::Normal,
                                fragment_id: Some(fragment.id.clone()),
                                pipeline_id: Some(pipeline.id.clone()),
                                time_percentage: None,
                                rows,
                                is_most_consuming: false,
                                is_second_most_consuming: false,
                                unique_metrics: operator.unique_metrics.clone(),
                                has_diagnostic: false,
                                diagnostic_ids: Vec::new(),
                            };

                            nodes.push(sink_node);
                            next_sink_id -= 1;
                        }
                    }
                }
            }
        }
    }

    /// Build nodes from fragments only (fallback)
    fn build_nodes_from_fragments(
        &self,
        text: &str,
        fragments: &[Fragment],
    ) -> ParseResult<Vec<ExecutionTreeNode>> {
        let mut nodes = Vec::new();
        let mut node_counter = 0;

        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    let plan_id = operator
                        .plan_node_id
                        .as_ref()
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(node_counter);

                    let operator_text =
                        OperatorParser::extract_operator_block(text, &operator.name, Some(plan_id));
                    if operator_text.is_empty() {
                        continue;
                    }

                    let pure_name = Self::extract_operator_name(&operator.name);
                    let mut metrics = MetricsParser::parse_common_metrics(&operator_text);
                    metrics.specialized = self.specialized_parser.parse(&pure_name, &operator_text);

                    let rows = metrics.push_row_num.or(metrics.pull_row_num);

                    let node = ExecutionTreeNode {
                        id: format!("node_{}", plan_id),
                        plan_node_id: Some(plan_id),
                        operator_name: pure_name,
                        node_type: OperatorParser::determine_node_type(&operator.name),
                        parent_plan_node_id: None,
                        children: Vec::new(),
                        depth: 0,
                        metrics,
                        is_hotspot: false,
                        hotspot_severity: HotSeverity::Normal,
                        fragment_id: Some(fragment.id.clone()),
                        pipeline_id: Some(pipeline.id.clone()),
                        time_percentage: None,
                        rows,
                        is_most_consuming: false,
                        is_second_most_consuming: false,
                        unique_metrics: HashMap::new(),
                        has_diagnostic: false,
                        diagnostic_ids: Vec::new(),
                    };

                    nodes.push(node);
                    node_counter += 1;
                }
            }
        }

        Ok(nodes)
    }

    /// Build operator text for specialized parsing
    fn build_operator_text(
        pure_name: &str,
        plan_id: i32,
        operator: &crate::services::profile_analyzer::models::Operator,
    ) -> String {
        let mut text = String::new();
        text.push_str(&format!("{} (plan_node_id={}):\n", pure_name, plan_id));
        text.push_str("  CommonMetrics:\n");
        for (key, value) in &operator.common_metrics {
            text.push_str(&format!("     - {}: {}\n", key, value));
        }
        text.push_str("  UniqueMetrics:\n");
        for (key, value) in &operator.unique_metrics {
            text.push_str(&format!("     - {}: {}\n", key, value));
        }
        text
    }

    /// Extract operator name without plan_node_id suffix
    fn extract_operator_name(full_name: &str) -> String {
        if let Some(pos) = full_name.find(" (plan_node_id=") {
            full_name[..pos].trim().to_string()
        } else {
            full_name.trim().to_string()
        }
    }

    /// Aggregate metrics from multiple operator instances
    fn aggregate_operators(
        operators: &[&crate::services::profile_analyzer::models::Operator],
        topology_name: &str,
    ) -> crate::services::profile_analyzer::models::Operator {
        if operators.is_empty() {
            panic!("Empty operators list");
        }

        // Find matching operators
        let mut matching_operators: Vec<&crate::services::profile_analyzer::models::Operator> =
            Vec::new();

        for &op in operators {
            let op_name = Self::extract_operator_name(&op.name);
            let op_canonical = OperatorParser::canonical_topology_name(&op_name);
            if op_canonical == topology_name {
                matching_operators.push(op);
            }
        }

        // Fallback to normalized comparison
        if matching_operators.is_empty() {
            let normalized_topology = topology_name.to_uppercase().replace("-", "_");
            for &op in operators {
                let op_name = Self::extract_operator_name(&op.name);
                let op_normalized = op_name.to_uppercase().replace("-", "_");
                if op_normalized == normalized_topology {
                    matching_operators.push(op);
                }
            }
        }

        // Handle SCAN topology to actual operator mapping
        // Topology may show OLAP_SCAN, HDFS_SCAN, etc. but actual operator is CONNECTOR_SCAN
        if matching_operators.is_empty() {
            let scan_types = [
                "OLAP_SCAN",
                "HDFS_SCAN",
                "HUDI_SCAN",
                "ICEBERG_SCAN",
                "DELTA_SCAN",
                "PAIMON_SCAN",
                "FILE_SCAN",
            ];
            if scan_types.contains(&topology_name) {
                for &op in operators {
                    let op_name = Self::extract_operator_name(&op.name);
                    let op_canonical = OperatorParser::canonical_topology_name(&op_name);
                    if op_canonical == "CONNECTOR_SCAN" || op_canonical == topology_name {
                        matching_operators.push(op);
                    }
                }
            }
        }

        // Use first operator as fallback
        if matching_operators.is_empty() {
            matching_operators.push(operators[0]);
        }

        let mut base_operator = matching_operators[0].clone();

        // Aggregate time metrics
        let mut total_time_ns: u64 = 0;
        for &op in &matching_operators {
            if let Some(time_str) = op.common_metrics.get("OperatorTotalTime")
                && let Some(time_ms) = Self::parse_time_to_ms(time_str)
            {
                total_time_ns += (time_ms * 1_000_000.0) as u64;
            }
        }

        if total_time_ns > 0 {
            let total_time_ms = total_time_ns as f64 / 1_000_000.0;
            base_operator
                .common_metrics
                .insert("OperatorTotalTime".to_string(), format!("{}ms", total_time_ms));
        }

        // Aggregate count metrics
        let count_metrics = ["PushChunkNum", "PushRowNum", "PullChunkNum", "PullRowNum"];
        for metric_name in &count_metrics {
            let mut total_count: u64 = 0;
            for &op in &matching_operators {
                if let Some(count_str) = op.common_metrics.get(*metric_name)
                    && let Ok(count) = count_str.parse::<u64>()
                {
                    total_count += count;
                }
            }
            if total_count > 0 {
                base_operator
                    .common_metrics
                    .insert(metric_name.to_string(), total_count.to_string());
            }
        }

        // Aggregate unique metrics
        let mut aggregated_unique_metrics = HashMap::new();
        for &op in &matching_operators {
            for (key, value) in &op.unique_metrics {
                aggregated_unique_metrics.insert(key.clone(), value.clone());
            }
        }
        base_operator.unique_metrics = aggregated_unique_metrics;

        base_operator
    }

    /// Parse time string to milliseconds
    fn parse_time_to_ms(time_str: &str) -> Option<f64> {
        let time_str = time_str.trim();

        if time_str.ends_with("ms") {
            return time_str.trim_end_matches("ms").parse::<f64>().ok();
        }
        if time_str.ends_with("us") {
            return time_str
                .trim_end_matches("us")
                .parse::<f64>()
                .map(|us| us / 1000.0)
                .ok();
        }
        if time_str.ends_with("ns") {
            return time_str
                .trim_end_matches("ns")
                .parse::<f64>()
                .map(|ns| ns / 1_000_000.0)
                .ok();
        }
        if time_str.ends_with("s")
            && !time_str.ends_with("ms")
            && !time_str.ends_with("us")
            && !time_str.ends_with("ns")
        {
            return time_str
                .trim_end_matches("s")
                .parse::<f64>()
                .map(|s| s * 1000.0)
                .ok();
        }

        time_str.parse::<f64>().ok()
    }

    /// Compute top time-consuming nodes
    fn compute_top_time_consuming_nodes(nodes: &[ExecutionTreeNode], limit: usize) -> Vec<TopNode> {
        let mut sorted_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| {
                n.time_percentage.is_some()
                    && n.time_percentage.unwrap() > 0.0
                    && n.plan_node_id.is_some()
            })
            .collect();

        sorted_nodes.sort_by(|a, b| {
            let a_pct = a.time_percentage.unwrap_or(0.0);
            let b_pct = b.time_percentage.unwrap_or(0.0);
            b_pct
                .partial_cmp(&a_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sorted_nodes
            .iter()
            .take(limit)
            .enumerate()
            .map(|(i, node)| {
                let percentage = node.time_percentage.unwrap_or(0.0);
                TopNode {
                    rank: (i + 1) as u32,
                    operator_name: node.operator_name.clone(),
                    plan_node_id: node.plan_node_id.unwrap_or(-1),
                    total_time: node
                        .metrics
                        .operator_total_time_raw
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    time_percentage: percentage,
                    is_most_consuming: percentage > time_thresholds::MOST_CONSUMING_THRESHOLD,
                    is_second_most_consuming: percentage
                        > time_thresholds::SECOND_CONSUMING_THRESHOLD
                        && percentage <= time_thresholds::MOST_CONSUMING_THRESHOLD,
                }
            })
            .collect()
    }

    /// Analyze profile completeness by checking for MissingInstanceIds
    /// This detects when profile data is incomplete due to async collection
    fn analyze_profile_completeness(text: &str, summary: &mut ProfileSummary) {
        use once_cell::sync::Lazy;
        use regex::Regex;

        // Match Fragment sections with MissingInstanceIds
        static MISSING_INSTANCE_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"MissingInstanceIds:\s*([^\n]+)").unwrap());
        // Match Fragment headers to count total fragments
        static FRAGMENT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"Fragment \d+:").unwrap());

        // Count total fragments (each Fragment section represents a stage)
        let total_fragments = FRAGMENT_REGEX.find_iter(text).count();

        // Count fragments with missing instances
        let missing_fragments = MISSING_INSTANCE_REGEX.find_iter(text).count();

        // Update summary with completeness info
        summary.total_instance_count =
            if total_fragments > 0 { Some(total_fragments as i32) } else { None };
        summary.missing_instance_count =
            if missing_fragments > 0 { Some(missing_fragments as i32) } else { None };

        // Determine if profile is complete
        let is_complete = missing_fragments == 0;
        summary.is_profile_complete = Some(is_complete);

        // Generate warning message if incomplete
        if !is_complete && total_fragments > 0 {
            let missing_pct =
                (missing_fragments as f64 / total_fragments as f64 * 100.0).round() as i32;
            summary.profile_completeness_warning = Some(format!(
                "Profile 数据不完整: {} 个 Fragment 中有 {} 个 ({}%) 的执行数据缺失，建议稍后重新查询",
                total_fragments, missing_fragments, missing_pct
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_topology_json() {
        let text = r#"  - Topology: {"rootId": 1, "nodes": [{"id": 1, "name": "TEST"}]}"#;
        let json = ProfileComposer::extract_topology_json(text).unwrap();
        assert!(json.contains("rootId"));
    }

    #[test]
    fn test_extract_operator_name() {
        assert_eq!(
            ProfileComposer::extract_operator_name("OLAP_SCAN (plan_node_id=0)"),
            "OLAP_SCAN"
        );
        assert_eq!(ProfileComposer::extract_operator_name("HASH_JOIN"), "HASH_JOIN");
    }
}
