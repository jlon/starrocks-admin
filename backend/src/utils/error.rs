use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

/// API Error with rich context and automatic error trait implementations
///
/// Design: Uses thiserror for ergonomic error handling with context.
/// Each variant carries meaningful context to help with debugging.
#[derive(Error, Debug)]
pub enum ApiError {
    // Authentication errors 1xxx
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid credentials")]
    InvalidCredentials,

    // Cluster errors 2xxx
    #[error("Cluster {cluster_id} not found")]
    ClusterNotFound { cluster_id: i64 },

    #[error("Failed to connect to cluster: {message}")]
    ClusterConnectionFailed { message: String },

    #[error("Cluster operation timeout")]
    ClusterTimeout,

    #[error("Cluster authentication failed")]
    ClusterAuthFailed,

    // Resource errors 3xxx
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Query {query_id} not found")]
    QueryNotFound { query_id: String },

    #[error("Failed to kill query: {0}")]
    QueryKillFailed(String),

    // Validation errors 4xxx
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // System errors 5xxx
    #[error("Internal error: {0}")]
    InternalError(String),

    // System Function errors 6xxx
    #[error("System function not found: {0}")]
    SystemFunctionNotFound(String),

    #[error("System function duplicate")]
    SystemFunctionDuplicate,

    #[error("Category full: {0}")]
    CategoryFull(String),

    #[error("Invalid SQL: {0}")]
    InvalidSQL(String),

    #[error("SQL safety violation: {0}")]
    SQLSafetyViolation(String),

    #[error("Category cannot be deleted")]
    CategoryCannotDelete,

    // Database errors - auto-convert from sqlx::Error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    // Generic wrapper for other errors - auto-convert from anyhow::Error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ApiError {
    /// Helper to create unauthorized error
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    /// Helper to create cluster not found error
    pub fn cluster_not_found(cluster_id: i64) -> Self {
        Self::ClusterNotFound { cluster_id }
    }

    /// Helper to create cluster connection failed error
    pub fn cluster_connection_failed(message: impl Into<String>) -> Self {
        Self::ClusterConnectionFailed { message: message.into() }
    }

    /// Helper to create invalid credentials error
    pub fn invalid_credentials() -> Self {
        Self::InvalidCredentials
    }

    /// Helper to create internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError(message.into())
    }

    /// Helper to create invalid data error
    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    /// Helper to create database error (for non-sqlx errors)
    pub fn database_error(err: impl std::fmt::Display) -> Self {
        Self::InternalError(format!("Database error: {}", err))
    }

    /// Helper to create validation error
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::ValidationError(message.into())
    }

    /// Helper to create not found error
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::ResourceNotFound(message.into())
    }

    /// Helper to create forbidden error (uses Unauthorized for compatibility)
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    /// Helper to create invalid SQL error
    pub fn invalid_sql(message: impl Into<String>) -> Self {
        Self::InvalidSQL(message.into())
    }

    /// Helper to create category full error
    pub fn category_full(message: impl Into<String>) -> Self {
        Self::CategoryFull(message.into())
    }

    /// Helper to create SQL safety violation error
    pub fn sql_safety_violation(message: impl Into<String>) -> Self {
        Self::SQLSafetyViolation(message.into())
    }

    /// Get legacy error code for backward compatibility
    pub fn error_code(&self) -> i32 {
        match self {
            // Authentication errors 1xxx
            Self::Unauthorized(_) => 1001,
            Self::TokenExpired => 1002,
            Self::InvalidCredentials => 1003,

            // Cluster errors 2xxx
            Self::ClusterNotFound { .. } => 2001,
            Self::ClusterConnectionFailed { .. } => 2002,
            Self::ClusterTimeout => 2003,
            Self::ClusterAuthFailed => 2004,

            // Resource errors 3xxx
            Self::ResourceNotFound(_) => 3000,
            Self::QueryNotFound { .. } => 3001,
            Self::QueryKillFailed(_) => 3002,

            // Validation errors 4xxx
            Self::ValidationError(_) => 4001,
            Self::InvalidInput(_) => 4002,

            // System errors 5xxx
            Self::InternalError(_) => 5001,
            Self::Database(_) => 5002,
            Self::Other(_) => 5001,

            // System Function errors 6xxx
            Self::SystemFunctionNotFound(_) => 6001,
            Self::SystemFunctionDuplicate => 6002,
            Self::CategoryFull(_) => 6003,
            Self::InvalidSQL(_) => 6004,
            Self::SQLSafetyViolation(_) => 6005,
            Self::CategoryCannotDelete => 6006,
        }
    }
}

/// Legacy error response for backward compatibility
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let code = self.error_code();
        let message = self.to_string();

        let status = match code {
            1001..=1999 => StatusCode::UNAUTHORIZED,
            2001..=2999 => StatusCode::BAD_REQUEST,
            3000..=3999 => StatusCode::NOT_FOUND,
            4001..=4999 => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let response = ApiErrorResponse { code, message, details: None };

        (status, Json(response)).into_response()
    }
}

/// Implement From for serde_json::Error
impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::internal_error(format!("JSON serialization error: {}", err))
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
