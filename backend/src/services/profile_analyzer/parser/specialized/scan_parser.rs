//! Scan operator specialized metrics parser

use crate::services::profile_analyzer::models::{OperatorSpecializedMetrics, ScanMetrics};
use crate::services::profile_analyzer::parser::core::ValueParser;
use once_cell::sync::Lazy;
use regex::Regex;

static TABLE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"Table:\s*(\S+)").unwrap());

static ROLLUP_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"Rollup:\s*(\S+)").unwrap());

/// Parser for scan operator metrics
#[derive(Debug, Clone, Default)]
pub struct ScanMetricsParser;

impl ScanMetricsParser {
    /// Parse OLAP_SCAN operator metrics
    pub fn parse_olap_scan(&self, text: &str) -> OperatorSpecializedMetrics {
        let metrics = self.parse_common_scan_metrics(text);
        OperatorSpecializedMetrics::OlapScan(metrics)
    }

    /// Parse CONNECTOR_SCAN operator metrics
    pub fn parse_connector_scan(&self, text: &str) -> OperatorSpecializedMetrics {
        let metrics = self.parse_common_scan_metrics(text);
        OperatorSpecializedMetrics::ConnectorScan(metrics)
    }

    /// Parse common scan metrics
    fn parse_common_scan_metrics(&self, text: &str) -> ScanMetrics {
        let mut metrics = ScanMetrics::default();

        // Extract table name
        if let Some(cap) = TABLE_REGEX.captures(text) {
            metrics.table = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        }

        // Extract rollup
        if let Some(cap) = ROLLUP_REGEX.captures(text) {
            metrics.rollup = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        }

        // Parse metrics from lines
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
                    "SharedScan" => {
                        metrics.shared_scan = value.to_lowercase() == "true";
                    },
                    "ScanTime" => {
                        if let Ok(duration) = ValueParser::parse_duration(value) {
                            metrics.scan_time_ns = Some(duration.as_nanos() as u64);
                        }
                    },
                    "IOTime" => {
                        if let Ok(duration) = ValueParser::parse_duration(value) {
                            metrics.io_time_ns = Some(duration.as_nanos() as u64);
                        }
                    },
                    "BytesRead" | "CompressedBytesRead" => {
                        if let Ok(bytes) = ValueParser::parse_bytes(value) {
                            metrics.bytes_read = Some(bytes);
                        }
                    },
                    "RowsRead" | "RawRowsRead" => {
                        if let Ok(rows) = ValueParser::parse_number::<u64>(value) {
                            metrics.rows_read = Some(rows);
                        }
                    },
                    _ => {},
                }
            }
        }

        metrics
    }
}
