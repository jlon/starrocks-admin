//! Fragment parser for StarRocks profile
//!
//! Parses Fragment and Pipeline structures from profile text.

use crate::services::profile_analyzer::models::{Fragment, Operator, Pipeline};
use crate::services::profile_analyzer::parser::core::{MetricsParser, OperatorParser};
use crate::services::profile_analyzer::parser::error::ParseResult;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

static FRAGMENT_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*Fragment\s+(\d+):").unwrap());

static PIPELINE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*Pipeline\s+\(id=(\d+)\):").unwrap());

/// Parser for Fragment and Pipeline structures
pub struct FragmentParser;

impl FragmentParser {
    /// Parse a single fragment from text
    pub fn parse_fragment(text: &str, id: &str) -> ParseResult<Fragment> {
        let backend_addresses = Self::extract_backend_addresses(text);
        let instance_ids = Self::extract_instance_ids(text);
        let pipelines = Self::parse_pipelines(text)?;

        Ok(Fragment { id: id.to_string(), backend_addresses, instance_ids, pipelines })
    }

    /// Extract all fragments from profile text
    pub fn extract_all_fragments(text: &str) -> Vec<Fragment> {
        let mut fragments = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            if let Some(caps) = FRAGMENT_REGEX.captures(line.trim()) {
                let id = caps.get(1).unwrap().as_str().to_string();
                let start_idx = i;
                let base_indent = Self::get_indent(line);

                // Find end of fragment (next fragment at same indent level)
                let mut end_idx = lines.len();
                for (j, line) in lines.iter().enumerate().skip(i + 1) {
                    let next_indent = Self::get_indent(line);
                    if next_indent <= base_indent && FRAGMENT_REGEX.is_match(line.trim()) {
                        end_idx = j;
                        break;
                    }
                }

                let fragment_text = lines[start_idx..end_idx].join("\n");

                if let Ok(fragment) = Self::parse_fragment(&fragment_text, &id) {
                    fragments.push(fragment);
                }

                i = end_idx;
            } else {
                i += 1;
            }
        }

        fragments
    }

    /// Parse pipelines from fragment text
    fn parse_pipelines(text: &str) -> ParseResult<Vec<Pipeline>> {
        let mut pipelines = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            if let Some(caps) = PIPELINE_REGEX.captures(line.trim()) {
                let id = caps.get(1).unwrap().as_str().to_string();
                let start_idx = i;
                let base_indent = Self::get_indent(line);

                // Find end of pipeline
                let mut end_idx = lines.len();
                for (j, line) in lines.iter().enumerate().skip(i + 1) {
                    let next_indent = Self::get_indent(line);
                    if next_indent <= base_indent
                        && (PIPELINE_REGEX.is_match(line.trim())
                            || FRAGMENT_REGEX.is_match(line.trim()))
                    {
                        end_idx = j;
                        break;
                    }
                }

                let pipeline_text = lines[start_idx..end_idx].join("\n");
                let pipeline = Self::parse_single_pipeline(&pipeline_text, &id)?;
                pipelines.push(pipeline);
                i = end_idx;
            } else {
                i += 1;
            }
        }

        Ok(pipelines)
    }

    /// Parse a single pipeline
    fn parse_single_pipeline(text: &str, id: &str) -> ParseResult<Pipeline> {
        let metrics = Self::extract_pipeline_metrics(text);
        let operators = Self::extract_operators(text);

        Ok(Pipeline { id: id.to_string(), metrics, operators })
    }

    /// Extract pipeline-level metrics
    fn extract_pipeline_metrics(text: &str) -> HashMap<String, String> {
        let mut metrics = HashMap::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") && trimmed.contains(": ") {
                let rest = trimmed.trim_start_matches("- ");
                let parts: Vec<&str> = rest.splitn(2, ": ").collect();
                if parts.len() == 2 {
                    metrics.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                }
            }
        }

        metrics
    }

    /// Extract operators from pipeline text
    fn extract_operators(text: &str) -> Vec<Operator> {
        let mut operators = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let trimmed = lines[i].trim();

            if OperatorParser::is_operator_header(trimmed) {
                let full_header = trimmed.trim_end_matches(':').to_string();

                // Extract operator name (without plan_node_id suffix)
                let operator_name = if let Some(pos) = full_header.find(" (plan_node_id=") {
                    full_header[..pos].to_string()
                } else {
                    full_header.clone()
                };

                let base_indent = Self::get_indent(lines[i]);

                // Collect operator lines
                let mut operator_lines = vec![lines[i]];
                i += 1;

                while i < lines.len() {
                    let line = lines[i];
                    if line.trim().is_empty() {
                        i += 1;
                        continue;
                    }

                    let current_indent = Self::get_indent(line);
                    if current_indent <= base_indent {
                        break;
                    }

                    operator_lines.push(line);
                    i += 1;
                }

                let operator_text = operator_lines.join("\n");

                // Extract plan_node_id
                let plan_node_id = if full_header.contains("plan_node_id=") {
                    full_header
                        .split("plan_node_id=")
                        .nth(1)
                        .and_then(|s| s.trim_end_matches(')').parse::<i32>().ok())
                        .map(|n| n.to_string())
                } else {
                    None
                };

                // Parse metrics
                let common_metrics_text =
                    MetricsParser::extract_common_metrics_block(&operator_text);
                let unique_metrics_text =
                    MetricsParser::extract_unique_metrics_block(&operator_text);

                let common_metrics = Self::parse_metrics_to_hashmap(&common_metrics_text);
                let unique_metrics = Self::parse_metrics_to_hashmap(&unique_metrics_text);

                operators.push(Operator {
                    name: operator_name,
                    plan_node_id,
                    operator_id: None,
                    common_metrics,
                    unique_metrics,
                    children: Vec::new(),
                });
            } else {
                i += 1;
            }
        }

        operators
    }

    /// Parse metrics text to HashMap
    /// This function recursively parses nested metrics blocks like DataCache:, ORC:
    /// It flattens all nested metrics into a single level HashMap
    fn parse_metrics_to_hashmap(text: &str) -> HashMap<String, String> {
        let mut metrics = HashMap::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") {
                let rest = trimmed.trim_start_matches("- ");

                if let Some(colon_pos) = rest.find(": ") {
                    let key = rest[..colon_pos].trim().to_string();
                    let value = rest[colon_pos + 2..].trim().to_string();

                    // Skip empty values (these are section headers like "ORC: " with empty value)
                    if value.is_empty() {
                        continue;
                    }

                    // Include __MAX_OF_ metrics as they are needed for time percentage calculation
                    // Skip __MIN_OF_ metrics for cleaner output
                    if !key.starts_with("__MIN_OF_") {
                        metrics.insert(key, value);
                    }
                } else if !rest.is_empty() {
                    // This is a section header like "DataCache:" - skip it
                    // The nested metrics will be parsed in subsequent lines
                }
            }
        }

        metrics
    }

    /// Extract backend addresses from fragment text
    fn extract_backend_addresses(text: &str) -> Vec<String> {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- BackendAddresses:") {
                let addresses = trimmed.trim_start_matches("- BackendAddresses:").trim();
                return addresses.split(',').map(|s| s.trim().to_string()).collect();
            }
        }
        Vec::new()
    }

    /// Extract instance IDs from fragment text
    fn extract_instance_ids(text: &str) -> Vec<String> {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- InstanceIds:") {
                let ids = trimmed.trim_start_matches("- InstanceIds:").trim();
                return ids.split(',').map(|s| s.trim().to_string()).collect();
            }
        }
        Vec::new()
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
    fn test_extract_backend_addresses() {
        let text = "   - BackendAddresses: 192.168.1.1:9060, 192.168.1.2:9060";
        let addrs = FragmentParser::extract_backend_addresses(text);
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0], "192.168.1.1:9060");
    }
}
