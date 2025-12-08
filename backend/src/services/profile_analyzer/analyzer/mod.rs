//! Profile analyzer module
//!
//! Provides rule-based diagnostics for query profile analysis.

pub mod baseline;
pub mod baseline_cache;
pub mod root_cause;
pub mod rule_engine;
pub mod rules;
pub mod thresholds;

pub use baseline::{
    BaselineCalculator, PerformanceBaseline, QueryComplexity, AuditLogRecord,
};
pub use baseline_cache::{
    BaselineCacheManager, BaselineProvider, BaselineSource, BaselineRefreshConfig,
};
pub use root_cause::{RootCauseAnalysis, RootCauseAnalyzer};
pub use rule_engine::RuleEngine;
