//! SQL Diagnosis Scenario - LLM-enhanced SQL performance analysis

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::services::llm::{LLMAnalysisRequestTrait, LLMAnalysisResponseTrait, LLMScenario};

const PROMPT: &str = include_str!("sql_diag_prompt.md");

// ============================================================================
// Request
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlDiagReq {
    pub sql: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars: Option<serde_json::Value>,
}

impl LLMAnalysisRequestTrait for SqlDiagReq {
    fn scenario(&self) -> LLMScenario {
        LLMScenario::SqlOptimization
    }
    fn system_prompt(&self) -> String {
        PROMPT.into()
    }

    fn cache_key(&self) -> String {
        format!("sqldiag:{}", self.sql_hash())
    }

    fn sql_hash(&self) -> String {
        let mut h = DefaultHasher::new();
        self.sql
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .hash(&mut h);
        format!("{:x}", h.finish())
    }

    fn profile_hash(&self) -> String {
        self.explain
            .as_ref()
            .map(|e| {
                let mut h = DefaultHasher::new();
                e.hash(&mut h);
                format!("{:x}", h.finish())
            })
            .unwrap_or_else(|| "none".into())
    }
}

// ============================================================================
// Response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SqlDiagResp {
    #[serde(default)]
    pub sql: String,
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub perf_issues: Vec<PerfIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain_analysis: Option<ExplainAnalysis>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfIssue {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExplainAnalysis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_strategy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_rows: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<String>,
}

impl LLMAnalysisResponseTrait for SqlDiagResp {
    fn summary(&self) -> &str {
        &self.summary
    }
    fn confidence(&self) -> Option<f64> {
        Some(self.confidence)
    }
}
