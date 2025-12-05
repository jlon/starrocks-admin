//! Operator parser for StarRocks profile
//!
//! Handles operator identification and node type classification.

use crate::services::profile_analyzer::models::NodeType;
use once_cell::sync::Lazy;
use regex::Regex;

static OPERATOR_HEADER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Z_]+(?:\s+\(plan_node_id=\d+\))?:$").unwrap());

/// Parser for operator-related operations
pub struct OperatorParser;

impl OperatorParser {
    /// Check if a line is an operator header
    pub fn is_operator_header(line: &str) -> bool {
        let trimmed = line.trim();

        // Must end with ':'
        if !trimmed.ends_with(':') {
            return false;
        }

        // Check for known operator patterns
        if trimmed.contains("(plan_node_id=") {
            return true;
        }

        // Check against regex pattern
        OPERATOR_HEADER_REGEX.is_match(trimmed)
    }

    /// Determine node type from operator name
    pub fn determine_node_type(operator_name: &str) -> NodeType {
        let name = operator_name.to_uppercase();

        match name.as_str() {
            "OLAP_SCAN" => NodeType::OlapScan,
            "CONNECTOR_SCAN" => NodeType::ConnectorScan,
            "HASH_JOIN" | "NEST_LOOP_JOIN" | "NESTLOOP_JOIN" => NodeType::HashJoin,
            "AGGREGATE" | "AGGREGATION" => NodeType::Aggregate,
            "LIMIT" | "TOP_N" => NodeType::Limit,
            "EXCHANGE_SINK" | "LOCAL_EXCHANGE_SINK" => NodeType::ExchangeSink,
            "EXCHANGE" | "EXCHANGE_SOURCE" | "MERGE_EXCHANGE" => NodeType::ExchangeSource,
            "RESULT_SINK" => NodeType::ResultSink,
            "CHUNK_ACCUMULATE" => NodeType::ChunkAccumulate,
            "SORT" => NodeType::Sort,
            "PROJECT" => NodeType::Project,
            "TABLE_FUNCTION" => NodeType::TableFunction,
            "OLAP_TABLE_SINK" => NodeType::OlapTableSink,
            _ => NodeType::Unknown,
        }
    }

    /// Get canonical topology name for an operator
    ///
    /// Maps various operator names to their canonical form used in topology
    pub fn canonical_topology_name(operator_name: &str) -> String {
        let name = operator_name.to_uppercase();

        match name.as_str() {
            // Scan operators
            "OLAP_SCAN" | "OLAP_SCAN_OPERATOR" => "OLAP_SCAN".to_string(),
            "CONNECTOR_SCAN" | "CONNECTOR_SCAN_OPERATOR" => "CONNECTOR_SCAN".to_string(),

            // Join operators
            "HASH_JOIN" | "HASH_JOIN_BUILD" | "HASH_JOIN_PROBE" => "HASH_JOIN".to_string(),
            "NEST_LOOP_JOIN" | "NESTLOOP_JOIN" => "NESTLOOP_JOIN".to_string(),

            // Aggregate operators
            "AGGREGATE" | "AGGREGATION" | "AGGREGATE_BLOCKING" | "AGGREGATE_STREAMING" => {
                "AGGREGATE".to_string()
            },

            // Exchange operators
            // EXCHANGE_SINK contains NetworkTime which is needed for time calculation
            "EXCHANGE" | "EXCHANGE_SOURCE" | "EXCHANGE_SINK" | "MERGE_EXCHANGE" => {
                "EXCHANGE".to_string()
            },

            // Other operators - return as-is
            _ => name,
        }
    }

    /// Extract operator block from profile text
    pub fn extract_operator_block(
        text: &str,
        operator_name: &str,
        plan_node_id: Option<i32>,
    ) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut result = Vec::new();
        let mut in_operator = false;
        let mut base_indent = 0;

        for line in lines {
            let trimmed = line.trim();

            // Check if this is the target operator
            if !in_operator {
                let is_match = if let Some(plan_id) = plan_node_id {
                    trimmed.contains(operator_name)
                        && trimmed.contains(&format!("plan_node_id={}", plan_id))
                } else {
                    trimmed.starts_with(operator_name) && Self::is_operator_header(trimmed)
                };

                if is_match {
                    in_operator = true;
                    base_indent = Self::get_indent(line);
                    result.push(line);
                }
            } else {
                // Check if we've exited the operator block
                let current_indent = Self::get_indent(line);

                if !trimmed.is_empty() && current_indent <= base_indent {
                    // Check if this is a new operator
                    if Self::is_operator_header(trimmed) {
                        break;
                    }
                }

                if current_indent > base_indent || trimmed.is_empty() {
                    result.push(line);
                } else {
                    break;
                }
            }
        }

        result.join("\n")
    }

    /// Get indentation level of a line
    fn get_indent(line: &str) -> usize {
        line.chars().take_while(|c| c.is_whitespace()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_operator_header() {
        assert!(OperatorParser::is_operator_header("OLAP_SCAN (plan_node_id=0):"));
        assert!(OperatorParser::is_operator_header("HASH_JOIN (plan_node_id=5):"));
        assert!(!OperatorParser::is_operator_header("- OperatorTotalTime: 100ms"));
        assert!(!OperatorParser::is_operator_header("CommonMetrics:"));
    }

    #[test]
    fn test_determine_node_type() {
        assert_eq!(OperatorParser::determine_node_type("OLAP_SCAN"), NodeType::OlapScan);
        assert_eq!(OperatorParser::determine_node_type("HASH_JOIN"), NodeType::HashJoin);
        assert_eq!(OperatorParser::determine_node_type("EXCHANGE"), NodeType::ExchangeSource);
        assert_eq!(OperatorParser::determine_node_type("RESULT_SINK"), NodeType::ResultSink);
    }

    #[test]
    fn test_canonical_topology_name() {
        assert_eq!(OperatorParser::canonical_topology_name("OLAP_SCAN"), "OLAP_SCAN");
        assert_eq!(OperatorParser::canonical_topology_name("HASH_JOIN_BUILD"), "HASH_JOIN");
        assert_eq!(OperatorParser::canonical_topology_name("AGGREGATE_BLOCKING"), "AGGREGATE");
    }
}
