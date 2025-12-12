//! LLM Data Models
//!
//! Core data structures for LLM service, including providers, sessions, and results.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ============================================================================
// LLM Scenario Types
// ============================================================================

/// LLM analysis scenario type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LLMScenario {
    /// Root cause analysis for query profile
    RootCauseAnalysis,
    /// SQL optimization suggestions (future)
    SqlOptimization,
    /// Parameter tuning recommendations (future)
    ParameterTuning,
    /// Table DDL optimization (future)
    DdlOptimization,
    /// General Q&A about StarRocks (future)
    GeneralQa,
}

impl LLMScenario {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RootCauseAnalysis => "root_cause_analysis",
            Self::SqlOptimization => "sql_optimization",
            Self::ParameterTuning => "parameter_tuning",
            Self::DdlOptimization => "ddl_optimization",
            Self::GeneralQa => "general_qa",
        }
    }
}

// ============================================================================
// LLM Provider
// ============================================================================

/// LLM Provider configuration from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct LLMProvider {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub api_base: String,
    pub model_name: String,
    #[serde(skip_serializing)]
    pub api_key_encrypted: Option<String>,
    pub is_active: bool,
    pub max_tokens: i32,
    pub temperature: f64,
    pub timeout_seconds: i32,
    pub enabled: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Provider info for external display (without sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMProviderInfo {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub api_base: String,
    pub model_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_masked: Option<String>,
    pub is_active: bool,
    pub enabled: bool,
    pub max_tokens: i32,
    pub temperature: f64,
    pub timeout_seconds: i32,
    pub priority: i32,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl From<&LLMProvider> for LLMProviderInfo {
    fn from(p: &LLMProvider) -> Self {
        // Mask API key for display
        let api_key_masked = p.api_key_encrypted.as_ref().map(|key| {
            if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            } else {
                "****".to_string()
            }
        });

        Self {
            id: p.id,
            name: p.name.clone(),
            display_name: p.display_name.clone(),
            api_base: p.api_base.clone(),
            model_name: p.model_name.clone(),
            api_key_masked,
            is_active: p.is_active,
            enabled: p.enabled,
            max_tokens: p.max_tokens,
            temperature: p.temperature,
            timeout_seconds: p.timeout_seconds,
            priority: p.priority,
            created_at: p.created_at.to_rfc3339(),
            updated_at: Some(p.updated_at.to_rfc3339()),
        }
    }
}

/// Request to create a provider
#[derive(Debug, Clone, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String,
    pub display_name: String,
    pub api_base: String,
    pub model_name: String,
    pub api_key: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: i32,
    #[serde(default = "default_priority")]
    pub priority: i32,
}

/// Request to update a provider
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProviderRequest {
    pub display_name: Option<String>,
    pub api_base: Option<String>,
    pub model_name: Option<String>,
    pub api_key: Option<String>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f64>,
    pub timeout_seconds: Option<i32>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
}

/// Response for test connection
#[derive(Debug, Clone, Serialize)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<i64>,
}

fn default_max_tokens() -> i32 {
    4096
}
fn default_temperature() -> f64 {
    0.3
}
fn default_timeout() -> i32 {
    60
}
fn default_priority() -> i32 {
    100
}

// ============================================================================
// LLM Analysis Session
// ============================================================================

/// Analysis session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn parse_status(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "processing" => Self::Processing,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Failed,
        }
    }
}

/// LLM Analysis Session from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct LLMAnalysisSession {
    pub id: String,
    pub provider_id: Option<i64>,
    pub scenario: String,
    pub query_id: String,
    pub cluster_id: Option<i64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub latency_ms: Option<i32>,
    pub error_message: Option<String>,
    pub retry_count: i32,
}

impl LLMAnalysisSession {
    pub fn status_enum(&self) -> SessionStatus {
        SessionStatus::parse_status(&self.status)
    }
}

// ============================================================================
// LLM Analysis Request (stored for debugging)
// ============================================================================

/// Stored request for debugging and replay
#[derive(Debug, Clone, FromRow)]
pub struct LLMAnalysisRequest {
    pub id: i64,
    pub session_id: String,
    pub request_json: String,
    pub sql_hash: String,
    pub profile_hash: String,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// LLM Analysis Result
// ============================================================================

/// Stored analysis result
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct LLMAnalysisResult {
    pub id: i64,
    pub session_id: String,
    pub root_causes_json: String,
    pub causal_chains_json: String,
    pub recommendations_json: String,
    pub summary: String,
    pub hidden_issues_json: String,
    pub confidence_avg: Option<f64>,
    pub root_cause_count: Option<i32>,
    pub recommendation_count: Option<i32>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// LLM Cache
// ============================================================================

/// Cached LLM response
#[derive(Debug, Clone, FromRow)]
pub struct LLMCache {
    pub id: i64,
    pub cache_key: String,
    pub scenario: String,
    pub request_hash: String,
    pub response_json: String,
    pub hit_count: i32,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
}

// ============================================================================
// LLM Usage Statistics
// ============================================================================

/// Daily usage statistics
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct LLMUsageStats {
    pub id: i64,
    pub date: String,
    pub provider_id: Option<i64>,
    pub total_requests: i32,
    pub successful_requests: i32,
    pub failed_requests: i32,
    pub total_input_tokens: i32,
    pub total_output_tokens: i32,
    pub avg_latency_ms: Option<f64>,
    pub cache_hits: i32,
    pub estimated_cost_usd: Option<f64>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// LLM Error Types
// ============================================================================

/// LLM service errors
#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    #[error("No active LLM provider configured")]
    NoProviderConfigured,

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("LLM API error: {0}")]
    ApiError(String),

    #[error("LLM response parsing error: {0}")]
    ParseError(String),

    #[error("LLM timeout after {0}s")]
    Timeout(u64),

    #[error("LLM rate limited, retry after {0}s")]
    RateLimited(u64),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("LLM service disabled")]
    Disabled,
}

impl LLMError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout(_) | Self::RateLimited(_) | Self::ApiError(_))
    }
}
