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
    BaselineCalculator, PerformanceBaseline, QueryComplexity, 
    AdaptiveThresholdCalculator, AuditLogRecord, BaselineStats,
};
pub use baseline_cache::{
    BaselineCacheManager, BaselineProvider, BaselineSource,
    CachedBaseline, BaselineRefreshConfig,
};
pub use root_cause::{RootCauseAnalysis, RootCauseAnalyzer, RootCause, CausalChain};
pub use rule_engine::RuleEngine;
