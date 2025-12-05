//! Profile parser module
//!
//! Provides parsing capabilities for StarRocks query profiles.

pub mod composer;
pub mod core;
pub mod error;
pub mod specialized;

// Re-export commonly used items
pub use composer::ProfileComposer;
