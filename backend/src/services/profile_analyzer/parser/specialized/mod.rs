//! Specialized metrics parsers for different operator types
//!
//! Each operator type has unique metrics that require specialized parsing.

use crate::services::profile_analyzer::models::OperatorSpecializedMetrics;

mod aggregate_parser;
mod exchange_parser;
mod join_parser;
mod scan_parser;
mod sink_parser;

pub use aggregate_parser::AggregateMetricsParser;
pub use exchange_parser::ExchangeMetricsParser;
pub use join_parser::JoinMetricsParser;
pub use scan_parser::ScanMetricsParser;
pub use sink_parser::SinkMetricsParser;

/// Unified specialized metrics parser
#[derive(Debug, Clone, Default)]
pub struct SpecializedMetricsParser {
    scan: ScanMetricsParser,
    exchange: ExchangeMetricsParser,
    join: JoinMetricsParser,
    aggregate: AggregateMetricsParser,
    sink: SinkMetricsParser,
}

impl SpecializedMetricsParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse specialized metrics based on operator type
    pub fn parse(&self, operator_name: &str, text: &str) -> OperatorSpecializedMetrics {
        match operator_name.to_uppercase().as_str() {
            "OLAP_SCAN" => self.scan.parse_olap_scan(text),
            "CONNECTOR_SCAN" => self.scan.parse_connector_scan(text),
            "EXCHANGE_SINK" => self.exchange.parse_sink(text),
            "EXCHANGE" | "EXCHANGE_SOURCE" | "MERGE_EXCHANGE" => self.exchange.parse_source(text),
            "HASH_JOIN" | "NEST_LOOP_JOIN" | "NESTLOOP_JOIN" => self.join.parse(text),
            "AGGREGATE" | "AGGREGATION" => self.aggregate.parse(text),
            "RESULT_SINK" => self.sink.parse_result_sink(text),
            "OLAP_TABLE_SINK" => self.sink.parse_olap_table_sink(text),
            _ => OperatorSpecializedMetrics::None,
        }
    }
}
