//! Parser error types for profile analysis

use thiserror::Error;

/// Errors that can occur during profile parsing
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Section not found: {0}")]
    SectionNotFound(String),

    #[error("Invalid topology JSON: {0}")]
    TopologyError(String),

    #[error("Tree build error: {0}")]
    TreeError(String),

    #[error("Failed to parse number: {0}")]
    ParseNumberError(String),

    #[error("Failed to parse duration: {0}")]
    ParseDurationError(String),

    #[error("Failed to parse bytes: {0}")]
    ParseBytesError(String),
}

/// Result type alias for parser operations
pub type ParseResult<T> = Result<T, ParseError>;
