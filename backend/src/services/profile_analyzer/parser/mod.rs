//! Profile parser module
//! 
//! Provides parsing capabilities for StarRocks query profiles.

pub mod error;
pub mod core;
pub mod specialized;
pub mod composer;

// Re-export commonly used items
pub use composer::ProfileComposer;
