//! Profile analyzer module
//!
//! Provides rule-based diagnostics for query profile analysis.

pub mod root_cause;
pub mod rule_engine;
pub mod rules;
pub mod thresholds;

pub use root_cause::{RootCauseAnalysis, RootCauseAnalyzer, RootCause, CausalChain};
pub use rule_engine::RuleEngine;
