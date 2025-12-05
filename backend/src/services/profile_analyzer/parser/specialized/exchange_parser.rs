//! Exchange operator specialized metrics parser

use crate::services::profile_analyzer::models::{ExchangeSinkMetrics, OperatorSpecializedMetrics};
use crate::services::profile_analyzer::parser::core::ValueParser;

/// Parser for exchange operator metrics
#[derive(Debug, Clone, Default)]
pub struct ExchangeMetricsParser;

impl ExchangeMetricsParser {
    /// Parse EXCHANGE_SINK operator metrics
    pub fn parse_sink(&self, text: &str) -> OperatorSpecializedMetrics {
        let mut metrics = ExchangeSinkMetrics::default();

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
                    "PartType" => {
                        metrics.part_type = value.to_string();
                    },
                    "BytesSent" | "BytesPassThrough" => {
                        if let Ok(bytes) = ValueParser::parse_bytes(value) {
                            metrics.bytes_sent = Some(bytes);
                        }
                    },
                    "NetworkTime" | "OverallThroughput" => {
                        if let Ok(duration) = ValueParser::parse_duration(value) {
                            metrics.network_time_ns = Some(duration.as_nanos() as u64);
                        }
                    },
                    "DestFragmentIds" => {
                        metrics.dest_fragment_ids =
                            value.split(',').map(|s| s.trim().to_string()).collect();
                    },
                    "DestBeAddresses" => {
                        metrics.dest_be_addresses =
                            value.split(',').map(|s| s.trim().to_string()).collect();
                    },
                    _ => {},
                }
            }
        }

        OperatorSpecializedMetrics::ExchangeSink(metrics)
    }

    /// Parse EXCHANGE_SOURCE operator metrics (returns None as it uses common metrics)
    pub fn parse_source(&self, _text: &str) -> OperatorSpecializedMetrics {
        // Exchange source typically uses common metrics
        OperatorSpecializedMetrics::None
    }
}
