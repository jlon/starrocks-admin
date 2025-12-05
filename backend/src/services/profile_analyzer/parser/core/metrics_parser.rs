//! Metrics parsing utilities for StarRocks profile operators
//!
//! Handles extraction and parsing of CommonMetrics and UniqueMetrics blocks.

use crate::services::profile_analyzer::models::OperatorMetrics;
use crate::services::profile_analyzer::parser::core::ValueParser;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

static METRIC_LINE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*-\s+([A-Za-z_][A-Za-z0-9_]*)(?::\s+(.+))?$").unwrap());

/// Parser for operator metrics
pub struct MetricsParser;

impl MetricsParser {
    /// Parse common metrics from operator text block
    pub fn parse_common_metrics(text: &str) -> OperatorMetrics {
        Self::parse_metrics_from_text(text)
    }

    /// Create OperatorMetrics from a HashMap of key-value pairs
    pub fn from_hashmap(map: &HashMap<String, String>) -> OperatorMetrics {
        let mut metrics = OperatorMetrics::default();

        for (key, value) in map {
            Self::set_metric_value(&mut metrics, key, value);
        }

        metrics
    }

    /// Merge memory-related metrics from unique_metrics into OperatorMetrics
    /// This handles metrics like HashTableMemoryUsage, LocalExchangePeakMemoryUsage, etc.
    pub fn merge_memory_metrics(
        metrics: &mut OperatorMetrics,
        unique_metrics: &HashMap<String, String>,
    ) {
        for (key, value) in unique_metrics {
            // Only process memory-related metrics
            if key.contains("Memory") || key.contains("memory") {
                Self::set_metric_value(metrics, key, value);
            }
        }
    }

    /// Parse metrics from raw text
    pub fn parse_metrics_from_text(text: &str) -> OperatorMetrics {
        let mut metrics = OperatorMetrics::default();

        for line in text.lines() {
            if let Some((key, value)) = Self::parse_metric_line(line) {
                Self::set_metric_value(&mut metrics, &key, &value);
            }
        }

        metrics
    }

    /// Parse a single metric line
    ///
    /// Format: "- MetricName: value"
    pub fn parse_metric_line(line: &str) -> Option<(String, String)> {
        METRIC_LINE_REGEX.captures(line).and_then(|caps| {
            let key = caps.get(1)?.as_str().trim().to_string();
            let value = caps.get(2)?.as_str().trim().to_string();
            Some((key, value))
        })
    }

    /// Extract CommonMetrics block from operator text
    pub fn extract_common_metrics_block(text: &str) -> String {
        Self::extract_section_block(text, "CommonMetrics:")
    }

    /// Extract UniqueMetrics block from operator text
    pub fn extract_unique_metrics_block(text: &str) -> String {
        Self::extract_section_block(text, "UniqueMetrics:")
    }

    /// Extract a section block from text
    fn extract_section_block(text: &str, section_marker: &str) -> String {
        if let Some(start) = text.find(section_marker) {
            let after_marker = &text[start + section_marker.len()..];
            let lines: Vec<&str> = after_marker.lines().collect();

            if lines.is_empty() {
                return String::new();
            }

            let mut block_lines = Vec::new();
            // Find the indent of the section marker line itself
            let marker_line_start = text[..start].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let marker_line = &text[marker_line_start..start + section_marker.len()];
            let marker_indent = Self::get_indent_level(marker_line);

            for line in lines {
                let trimmed = line.trim();

                // Skip empty lines
                if trimmed.is_empty() {
                    block_lines.push(line);
                    continue;
                }

                let current_indent = Self::get_indent_level(line);

                // Stop at any section marker (CommonMetrics:, UniqueMetrics:) at same indent level
                if trimmed.ends_with("Metrics:") && current_indent <= marker_indent {
                    break;
                }

                // Stop at new operator (contains plan_node_id) at same or lower indent
                if trimmed.contains("(plan_node_id=") && !trimmed.starts_with("-") {
                    break;
                }

                // Stop at Pipeline section
                if trimmed.starts_with("Pipeline (id=") {
                    break;
                }

                // Stop at Fragment section
                if trimmed.starts_with("Fragment ") && trimmed.contains(":") {
                    break;
                }

                block_lines.push(line);
            }

            block_lines.join("\n")
        } else {
            String::new()
        }
    }

    /// Get indentation level of a line
    fn get_indent_level(line: &str) -> usize {
        line.len() - line.trim_start().len()
    }

    /// Set a metric value on OperatorMetrics based on key
    fn set_metric_value(metrics: &mut OperatorMetrics, key: &str, value: &str) {
        match key {
            "OperatorTotalTime" => {
                metrics.operator_total_time_raw = Some(value.to_string());
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.operator_total_time = Some(duration.as_nanos() as u64);
                }
            },
            "__MIN_OF_OperatorTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.operator_total_time_min = Some(duration.as_nanos() as u64);
                }
            },
            "__MAX_OF_OperatorTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.operator_total_time_max = Some(duration.as_nanos() as u64);
                }
            },
            "CPUTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.operator_total_time = Some(duration.as_nanos() as u64);
                }
            },
            "PushChunkNum" => {
                metrics.push_chunk_num = Self::extract_number(value);
            },
            "PushRowNum" => {
                metrics.push_row_num = Self::extract_number(value);
            },
            "PullChunkNum" => {
                metrics.pull_chunk_num = Self::extract_number(value);
            },
            "PullRowNum" => {
                metrics.pull_row_num = Self::extract_number(value);
            },
            "PushTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.push_total_time = Some(duration.as_nanos() as u64);
                }
            },
            "__MIN_OF_PushTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.push_total_time_min = Some(duration.as_nanos() as u64);
                }
            },
            "__MAX_OF_PushTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.push_total_time_max = Some(duration.as_nanos() as u64);
                }
            },
            "PullTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.pull_total_time = Some(duration.as_nanos() as u64);
                }
            },
            "__MIN_OF_PullTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.pull_total_time_min = Some(duration.as_nanos() as u64);
                }
            },
            "__MAX_OF_PullTotalTime" => {
                if let Ok(duration) = ValueParser::parse_duration(value) {
                    metrics.pull_total_time_max = Some(duration.as_nanos() as u64);
                }
            },
            "MemoryUsage" => {
                metrics.memory_usage = ValueParser::parse_bytes(value).ok();
            },
            // Handle various memory-related metrics and accumulate to memory_usage
            // These are operator-level peak memory metrics
            "LocalExchangePeakMemoryUsage"
            | "HashTableMemoryUsage"
            | "PeakChunkBufferMemoryUsage"
            | "PassThroughBufferPeakMemoryUsage"
            | "PeakBufferMemoryBytes"
            | "OperatorPeakMemoryUsage"
            | "AggregatorMemoryUsage"
            | "SortMemoryUsage" => {
                if let Ok(bytes) = ValueParser::parse_bytes(value) {
                    // Accumulate memory usage from various sources
                    let current = metrics.memory_usage.unwrap_or(0);
                    metrics.memory_usage = Some(current + bytes);
                }
            },
            "OutputChunkBytes" => {
                metrics.output_chunk_bytes = ValueParser::parse_bytes(value).ok();
            },
            _ => {
                // Unknown metric, ignore
            },
        }
    }

    fn extract_number(value: &str) -> Option<u64> {
        ValueParser::parse_number(value).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metric_line() {
        let line = "     - OperatorTotalTime: 123ms";
        let result = MetricsParser::parse_metric_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key, "OperatorTotalTime");
        assert_eq!(value, "123ms");
    }

    #[test]
    fn test_from_hashmap() {
        let mut map = HashMap::new();
        map.insert("OperatorTotalTime".to_string(), "100ms".to_string());
        map.insert("PushRowNum".to_string(), "1000".to_string());

        let metrics = MetricsParser::from_hashmap(&map);
        assert_eq!(metrics.operator_total_time, Some(100_000_000));
        assert_eq!(metrics.push_row_num, Some(1000));
    }
}
