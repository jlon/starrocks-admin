//! Sink operator specialized metrics parser

use crate::services::profile_analyzer::models::{OperatorSpecializedMetrics, ResultSinkMetrics};
use crate::services::profile_analyzer::parser::core::ValueParser;

/// Parser for sink operator metrics
#[derive(Debug, Clone, Default)]
pub struct SinkMetricsParser;

impl SinkMetricsParser {
    /// Parse RESULT_SINK operator metrics
    pub fn parse_result_sink(&self, text: &str) -> OperatorSpecializedMetrics {
        let mut metrics =
            ResultSinkMetrics { sink_type: "RESULT".to_string(), ..Default::default() };

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
                    "SinkType" => {
                        metrics.sink_type = value.to_string();
                    },
                    "AppendChunkTime" => {
                        if let Ok(duration) = ValueParser::parse_duration(value) {
                            metrics.append_chunk_time_ns = Some(duration.as_nanos() as u64);
                        }
                    },
                    "ResultSendTime" | "ResultRenderTime" => {
                        if let Ok(duration) = ValueParser::parse_duration(value) {
                            metrics.result_send_time_ns = Some(duration.as_nanos() as u64);
                        }
                    },
                    _ => {},
                }
            }
        }

        OperatorSpecializedMetrics::ResultSink(metrics)
    }

    /// Parse OLAP_TABLE_SINK operator metrics
    pub fn parse_olap_table_sink(&self, text: &str) -> OperatorSpecializedMetrics {
        let mut metrics =
            ResultSinkMetrics { sink_type: "OLAP_TABLE".to_string(), ..Default::default() };

        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("- ") {
                continue;
            }

            let rest = trimmed.trim_start_matches("- ");
            if let Some(colon_pos) = rest.find(": ") {
                let key = &rest[..colon_pos];
                let value = &rest[colon_pos + 2..];

                if key == "AppendChunkTime"
                    && let Ok(duration) = ValueParser::parse_duration(value)
                {
                    metrics.append_chunk_time_ns = Some(duration.as_nanos() as u64);
                }
            }
        }

        OperatorSpecializedMetrics::ResultSink(metrics)
    }
}
