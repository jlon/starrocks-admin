//! Profile analyzer module
//!
//! Provides rule-based diagnostics for query profile analysis.

pub mod baseline;
pub mod baseline_cache;
pub mod query_history;
pub mod root_cause;
pub mod rule_engine;
pub mod rules;
pub mod thresholds;

pub use baseline::{AuditLogRecord, BaselineCalculator, PerformanceBaseline, QueryComplexity};
pub use baseline_cache::{
    BaselineCacheManager, BaselineDriftResult, BaselineProvider, BaselineRefreshConfig,
    BaselineSource, DriftDetail, DriftDirection,
};
pub use query_history::{QUERY_HISTORY, QueryFingerprint, QueryHistoryService};
pub use root_cause::{RootCauseAnalysis, RootCauseAnalyzer};
pub use rule_engine::RuleEngine;
