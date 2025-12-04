//! Profile analyzer module
//! 
//! Provides rule-based diagnostics for query profile analysis.

pub mod rules;
pub mod rule_engine;

pub use rule_engine::RuleEngine;
