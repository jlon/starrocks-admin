//! Topology parser for StarRocks execution plan
//!
//! Parses the topology JSON from the Execution section to build the DAG structure.

use crate::services::profile_analyzer::models::{Fragment, TopologyGraph, TopologyNode};
use crate::services::profile_analyzer::parser::error::{ParseError, ParseResult};
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

/// Parser for execution topology
pub struct TopologyParser;

impl TopologyParser {
    /// Parse topology JSON with fragment information
    pub fn parse_with_fragments(
        json_str: &str,
        profile_text: &str,
        fragments: &[Fragment],
    ) -> ParseResult<TopologyGraph> {
        let json = Self::extract_json(json_str)?;

        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| ParseError::TopologyError(format!("Invalid JSON: {}", e)))?;

        let root_id = value
            .get("rootId")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| ParseError::TopologyError("Missing rootId".to_string()))?
            as i32;

        let nodes_array = value
            .get("nodes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ParseError::TopologyError("Missing nodes array".to_string()))?;

        let mut nodes = Vec::new();
        for node_value in nodes_array {
            let node = Self::parse_node(node_value)?;
            nodes.push(node);
        }

        // Add sink nodes from fragments
        Self::extract_and_add_sink_nodes(&mut nodes, profile_text, fragments, root_id)?;

        Ok(TopologyGraph { root_id, nodes })
    }

    /// Extract and add sink nodes that are not in the topology
    fn extract_and_add_sink_nodes(
        nodes: &mut Vec<TopologyNode>,
        _profile_text: &str,
        fragments: &[Fragment],
        _root_id: i32,
    ) -> ParseResult<()> {
        let selected_sink = Self::select_sink_node(fragments);

        if let Some(sink_name) = selected_sink {
            let sink_plan_id = Self::find_sink_plan_node_id(fragments, &sink_name);
            let sink_id = sink_plan_id.unwrap_or(-1);

            if !nodes.iter().any(|n| n.id == sink_id) {
                let sink_node = TopologyNode {
                    id: sink_id,
                    name: sink_name,
                    properties: HashMap::new(),
                    children: vec![],
                };
                nodes.push(sink_node);
            }
        }

        Ok(())
    }

    /// Extract operator name without plan_node_id suffix
    fn extract_operator_name(full_name: &str) -> String {
        if let Some(pos) = full_name.find(" (plan_node_id=") {
            full_name[..pos].to_string()
        } else {
            full_name.to_string()
        }
    }

    /// Select the most appropriate sink node from fragments
    fn select_sink_node(fragments: &[Fragment]) -> Option<String> {
        let mut sink_candidates = Vec::new();

        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    let pure_name = Self::extract_operator_name(&operator.name);
                    if pure_name.ends_with("_SINK") {
                        let is_final_sink = Self::is_final_sink(&pure_name);
                        let priority = Self::get_sink_priority(&pure_name);
                        sink_candidates.push((pure_name, is_final_sink, priority));
                    }
                }
            }
        }

        // Sort by: final sink first, then by priority
        sink_candidates.sort_by(|a, b| match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.2.cmp(&b.2),
        });

        sink_candidates.first().map(|(name, _, _)| name.clone())
    }

    /// Check if a sink is a final sink (not intermediate)
    fn is_final_sink(sink_name: &str) -> bool {
        if sink_name.contains("EXCHANGE_SINK") || sink_name.contains("LOCAL_EXCHANGE_SINK") {
            return false;
        }
        if sink_name.contains("MULTI_CAST") {
            return false;
        }
        true
    }

    /// Get priority for sink node selection (lower is better)
    fn get_sink_priority(sink_name: &str) -> i32 {
        match sink_name {
            "RESULT_SINK" => 1,
            "OLAP_TABLE_SINK" => 2,
            name if name.contains("TABLE_SINK") => 3,
            name if name.contains("EXCHANGE_SINK") => 4,
            name if name.contains("LOCAL_EXCHANGE_SINK") => 5,
            _ => 6,
        }
    }

    /// Find plan_node_id for a sink operator
    fn find_sink_plan_node_id(fragments: &[Fragment], sink_name: &str) -> Option<i32> {
        for fragment in fragments {
            for pipeline in &fragment.pipelines {
                for operator in &pipeline.operators {
                    if operator.name == sink_name
                        && let Some(plan_id) = &operator.plan_node_id
                        && let Ok(plan_id_int) = plan_id.parse::<i32>()
                    {
                        return Some(plan_id_int);
                    }
                }
            }
        }
        None
    }

    /// Extract JSON from string (handles prefix text)
    fn extract_json(s: &str) -> ParseResult<&str> {
        let s = s.trim();

        if let Some(start) = s.find('{') {
            let mut depth = 0;
            let mut end = start;

            for (i, ch) in s[start..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i + 1;
                            break;
                        }
                    },
                    _ => {},
                }
            }

            if depth == 0 {
                return Ok(&s[start..end]);
            }
        }

        Err(ParseError::TopologyError("No valid JSON found".to_string()))
    }

    /// Parse a single topology node from JSON
    fn parse_node(value: &serde_json::Value) -> ParseResult<TopologyNode> {
        let id = value
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| ParseError::TopologyError("Node missing id".to_string()))?
            as i32;

        let name = value
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ParseError::TopologyError("Node missing name".to_string()))?
            .to_string();

        let properties = value
            .get("properties")
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let children = value
            .get("children")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64().map(|i| i as i32))
                    .collect()
            })
            .unwrap_or_default();

        Ok(TopologyNode { id, name, properties, children })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl TopologyParser {
        /// Parse topology JSON without fragment information (test only)
        pub fn parse(json_str: &str, profile_text: &str) -> ParseResult<TopologyGraph> {
            Self::parse_with_fragments(json_str, profile_text, &[])
        }

        /// Parse topology JSON only (test only)
        pub fn parse_without_profile(json_str: &str) -> ParseResult<TopologyGraph> {
            Self::parse(json_str, "")
        }

        /// Validate topology structure (test only)
        pub fn validate(topology: &TopologyGraph) -> ParseResult<()> {
            // Check root exists
            if !topology.nodes.iter().any(|n| n.id == topology.root_id) {
                return Err(ParseError::TopologyError(format!(
                    "Root node {} not found",
                    topology.root_id
                )));
            }

            let node_ids: HashSet<i32> = topology.nodes.iter().map(|n| n.id).collect();

            // Check all children exist
            for node in &topology.nodes {
                for child_id in &node.children {
                    if !node_ids.contains(child_id) {
                        return Err(ParseError::TopologyError(format!(
                            "Child node {} referenced but not found",
                            child_id
                        )));
                    }
                }
            }

            Ok(())
        }
    }

    #[test]
    fn test_parse_topology() {
        let json = r#"{
            "rootId": 1,
            "nodes": [
                {
                    "id": 1,
                    "name": "EXCHANGE",
                    "properties": {"sinkIds": []},
                    "children": [0]
                },
                {
                    "id": 0,
                    "name": "OLAP_SCAN",
                    "properties": {},
                    "children": []
                }
            ]
        }"#;

        let topology = TopologyParser::parse(json, "").unwrap();
        assert_eq!(topology.root_id, 1);
        assert_eq!(topology.nodes.len(), 2);
        assert_eq!(topology.nodes[0].name, "EXCHANGE");
    }

    #[test]
    fn test_validate_topology() {
        let topology = TopologyGraph {
            root_id: 1,
            nodes: vec![
                TopologyNode {
                    id: 1,
                    name: "ROOT".to_string(),
                    properties: HashMap::new(),
                    children: vec![0],
                },
                TopologyNode {
                    id: 0,
                    name: "LEAF".to_string(),
                    properties: HashMap::new(),
                    children: vec![],
                },
            ],
        };

        assert!(TopologyParser::validate(&topology).is_ok());
    }
}
