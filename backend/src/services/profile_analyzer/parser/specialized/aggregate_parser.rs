//! Aggregate operator specialized metrics parser

use crate::services::profile_analyzer::models::{AggregateMetrics, OperatorSpecializedMetrics};
use crate::services::profile_analyzer::parser::core::ValueParser;

/// Parser for aggregate operator metrics
#[derive(Debug, Clone, Default)]
pub struct AggregateMetricsParser;

impl AggregateMetricsParser {
    /// Parse aggregate operator metrics
    pub fn parse(&self, text: &str) -> OperatorSpecializedMetrics {
        let mut metrics = AggregateMetrics::default();

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
                    "AggMode" | "AggregateMode" => {
                        metrics.agg_mode = value.to_string();
                    },
                    "ChunkByChunk" => {
                        metrics.chunk_by_chunk = value.to_lowercase() == "true";
                    },
                    "InputRows" => {
                        if let Ok(rows) = ValueParser::parse_number::<u64>(value) {
                            metrics.input_rows = Some(rows);
                        }
                    },
                    "AggFunctionTime" | "AggComputeTime" => {
                        if let Ok(duration) = ValueParser::parse_duration(value) {
                            metrics.agg_function_time_ns = Some(duration.as_nanos() as u64);
                        }
                    },
                    _ => {},
                }
            }
        }

        OperatorSpecializedMetrics::Aggregate(metrics)
    }
}
