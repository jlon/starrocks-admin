//! Join operator specialized metrics parser

use crate::services::profile_analyzer::models::{JoinMetrics, OperatorSpecializedMetrics};
use crate::services::profile_analyzer::parser::core::ValueParser;

/// Parser for join operator metrics
#[derive(Debug, Clone, Default)]
pub struct JoinMetricsParser;

impl JoinMetricsParser {
    /// Parse join operator metrics
    pub fn parse(&self, text: &str) -> OperatorSpecializedMetrics {
        let mut metrics = JoinMetrics::default();

        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("- ") {
                continue;
            }

            let rest = trimmed.trim_start_matches("- ");
            if let Some(colon_pos) = rest.find(": ") {
                let key = &rest[..colon_pos];
                let value = &rest[colon_pos + 2..];

                match key {
                    "JoinType" => {
                        metrics.join_type = value.to_string();
                    },
                    "BuildRows" | "HashTableSize" => {
                        if let Ok(rows) = ValueParser::parse_number::<u64>(value) {
                            metrics.build_rows = Some(rows);
                        }
                    },
                    "ProbeRows" | "InputRows" => {
                        if let Ok(rows) = ValueParser::parse_number::<u64>(value) {
                            metrics.probe_rows = Some(rows);
                        }
                    },
                    "RuntimeFilterNum" => {
                        if let Ok(num) = ValueParser::parse_number::<u64>(value) {
                            metrics.runtime_filter_num = Some(num);
                        }
                    },
                    _ => {},
                }
            }
        }

        OperatorSpecializedMetrics::Join(metrics)
    }
}
