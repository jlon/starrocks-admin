//! Parser error types for profile analysis

use thiserror::Error;

/// Errors that can occur during profile parsing
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid profile format: {0}")]
    InvalidFormat(String),
    
    #[error("Section not found: {0}")]
    SectionNotFound(String),
    
    #[error("Failed to parse value '{value}': {reason}")]
    ValueError { value: String, reason: String },
    
    #[error("Invalid topology JSON: {0}")]
    TopologyError(String),
    
    #[error("Operator parse error: {0}")]
    OperatorError(String),
    
    #[error("Tree build error: {0}")]
    TreeError(String),
    
    #[error("Metric parse error: {0}")]
    MetricError(String),
    
    #[error("Failed to parse number: {0}")]
    ParseNumberError(String),
    
    #[error("Failed to parse duration: {0}")]
    ParseDurationError(String),
    
    #[error("Failed to parse bytes: {0}")]
    ParseBytesError(String),
    
    #[error("Fragment parsing error: {0}")]
    FragmentError(String),
    
    #[error("Missing required data: {0}")]
    MissingData(String),
    
    #[error("Regex compile error: {0}")]
    RegexError(#[from] regex::Error),
    
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Internal parser error: {0}")]
    InternalError(String),
}

/// Result type alias for parser operations
pub type ParseResult<T> = Result<T, ParseError>;
