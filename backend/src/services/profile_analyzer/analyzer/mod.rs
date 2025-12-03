//! Profile analyzer module
//! 
//! Provides hotspot detection and suggestion generation.

pub mod hotspot_detector;
pub mod suggestion_engine;

pub use hotspot_detector::HotSpotDetector;
pub use suggestion_engine::SuggestionEngine;
