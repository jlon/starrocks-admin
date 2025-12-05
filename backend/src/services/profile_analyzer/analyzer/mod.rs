//! Profile analyzer module
//!
//! Provides rule-based diagnostics for query profile analysis.

pub mod rule_engine;
pub mod rules;

pub use rule_engine::RuleEngine;
