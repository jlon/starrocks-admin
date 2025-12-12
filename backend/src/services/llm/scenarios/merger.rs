//! Result Merger - Fuses rule engine diagnostics with LLM analysis
//!
//! Combines the deterministic results from rule engine with LLM's
//! open-domain reasoning to provide comprehensive root cause analysis.
//!
//! Note: Some structs/methods are defined here for modularity but may be
//! duplicated in models.rs for serialization. Allow dead_code for reserved APIs.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::root_cause::{
    LLMCausalChain, LLMHiddenIssue, LLMRecommendation, LLMRootCause, RootCauseAnalysisResponse,
};
use crate::services::profile_analyzer::models::{AggregatedDiagnostic, DiagnosticResult};

// ============================================================================
// Merged Result Types
// ============================================================================

/// Enhanced Profile Analysis Response (with LLM)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMEnhancedAnalysis {
    /// Whether LLM analysis is available
    pub available: bool,
    /// LLM analysis status: "pending" | "completed" | "failed" | "disabled"
    pub status: String,
    /// Root causes (may include implicit ones not detected by rules)
    #[serde(default)]
    pub root_causes: Vec<MergedRootCause>,
    /// Causal chains with explanations
    #[serde(default)]
    pub causal_chains: Vec<LLMCausalChain>,
    /// Merged recommendations (rule + LLM, deduplicated)
    #[serde(default)]
    pub merged_recommendations: Vec<MergedRecommendation>,
    /// Natural language summary
    #[serde(default)]
    pub summary: String,
    /// Hidden issues detected by LLM only
    #[serde(default)]
    pub hidden_issues: Vec<LLMHiddenIssue>,
    /// Whether this result was loaded from cache
    #[serde(default, skip_serializing_if = "is_false")]
    pub from_cache: bool,
    /// LLM analysis elapsed time in milliseconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_time_ms: Option<u64>,
}

/// Helper for serde skip_serializing_if
fn is_false(b: &bool) -> bool {
    !b
}

impl Default for LLMEnhancedAnalysis {
    fn default() -> Self {
        Self {
            available: false,
            status: "disabled".to_string(),
            root_causes: vec![],
            causal_chains: vec![],
            merged_recommendations: vec![],
            summary: String::new(),
            hidden_issues: vec![],
            from_cache: false,
            elapsed_time_ms: None,
        }
    }
}

impl LLMEnhancedAnalysis {
    /// Create a pending status
    pub fn pending() -> Self {
        Self { available: false, status: "pending".to_string(), ..Default::default() }
    }

    /// Create a failed status
    pub fn failed(error: &str) -> Self {
        Self { available: false, status: format!("failed: {}", error), ..Default::default() }
    }
}

/// Merged root cause (from rule engine and/or LLM)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedRootCause {
    /// Unique identifier
    pub id: String,
    /// Related rule IDs (if detected by rules)
    #[serde(default)]
    pub related_rule_ids: Vec<String>,
    /// Description
    pub description: String,
    /// Is this an implicit root cause (not detected by rules)?
    pub is_implicit: bool,
    /// Confidence score (1.0 for rule-based, 0.0-1.0 for LLM)
    pub confidence: f64,
    /// Source: "rule" | "llm" | "both"
    pub source: String,
    /// Evidence
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Symptoms caused by this root cause
    #[serde(default)]
    pub symptoms: Vec<String>,
}

/// Merged recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedRecommendation {
    /// Priority (1 = highest)
    pub priority: u32,
    /// Action description
    pub action: String,
    /// Expected improvement
    #[serde(default)]
    pub expected_improvement: String,
    /// SQL example (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql_example: Option<String>,
    /// Source: "rule" | "llm" | "both"
    pub source: String,
    /// Related root cause IDs
    #[serde(default)]
    pub related_root_causes: Vec<String>,
    /// Is this a root cause fix (vs symptom fix)?
    pub is_root_cause_fix: bool,
}

// ============================================================================
// Merger Implementation
// ============================================================================

/// Merge rule engine results with LLM analysis
pub struct ResultMerger;

impl ResultMerger {
    /// Merge rule diagnostics with LLM response
    pub fn merge(
        rule_diagnostics: &[AggregatedDiagnostic],
        llm_response: &RootCauseAnalysisResponse,
    ) -> LLMEnhancedAnalysis {
        let root_causes = Self::merge_root_causes(rule_diagnostics, &llm_response.root_causes);
        let recommendations =
            Self::merge_recommendations(rule_diagnostics, &llm_response.recommendations);

        LLMEnhancedAnalysis {
            available: true,
            status: "completed".to_string(),
            root_causes,
            causal_chains: llm_response.causal_chains.clone(),
            merged_recommendations: recommendations,
            summary: llm_response.summary.clone(),
            hidden_issues: llm_response.hidden_issues.clone(),
            from_cache: false,     // Will be set by caller if needed
            elapsed_time_ms: None, // Will be set by caller
        }
    }

    /// Merge root causes from rules and LLM
    fn merge_root_causes(
        rule_diagnostics: &[AggregatedDiagnostic],
        llm_root_causes: &[LLMRootCause],
    ) -> Vec<MergedRootCause> {
        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut merged: Vec<MergedRootCause> = llm_root_causes
            .iter()
            .map(|llm_rc| {
                seen_ids.insert(llm_rc.root_cause_id.clone());
                let related_rules: Vec<String> = llm_rc
                    .symptoms
                    .iter()
                    .filter(|s| rule_diagnostics.iter().any(|d| &d.rule_id == *s))
                    .cloned()
                    .collect();
                let source = if related_rules.is_empty() { "llm" } else { "both" };
                MergedRootCause {
                    id: llm_rc.root_cause_id.clone(),
                    related_rule_ids: related_rules,
                    description: llm_rc.description.clone(),
                    is_implicit: llm_rc.is_implicit,
                    confidence: llm_rc.confidence,
                    source: source.to_string(),
                    evidence: llm_rc.evidence.clone(),
                    symptoms: llm_rc.symptoms.clone(),
                }
            })
            .collect();

        // Add uncovered rule diagnostics as independent issues
        merged.extend(
            rule_diagnostics
                .iter()
                .filter(|diag| {
                    !llm_root_causes
                        .iter()
                        .any(|rc| rc.symptoms.contains(&diag.rule_id))
                })
                .filter_map(|diag| {
                    let id = format!("rule_{}", diag.rule_id);
                    seen_ids.insert(id.clone()).then_some(MergedRootCause {
                        id,
                        related_rule_ids: vec![diag.rule_id.clone()],
                        description: diag.message.clone(),
                        is_implicit: false,
                        confidence: 1.0, // Rule-based = 100% confidence
                        source: "rule".to_string(),
                        evidence: vec![diag.reason.clone()],
                        symptoms: vec![],
                    })
                }),
        );

        merged.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        merged
    }

    /// Merge recommendations from rules and LLM
    fn merge_recommendations(
        rule_diagnostics: &[AggregatedDiagnostic],
        llm_recommendations: &[LLMRecommendation],
    ) -> Vec<MergedRecommendation> {
        let mut seen_actions: HashSet<String> = HashSet::new();
        let mut merged: Vec<MergedRecommendation> = llm_recommendations
            .iter()
            .filter_map(|rec| {
                let action_key = Self::normalize_action(&rec.action);
                seen_actions
                    .insert(action_key)
                    .then_some(MergedRecommendation {
                        priority: rec.priority,
                        action: rec.action.clone(),
                        expected_improvement: rec.expected_improvement.clone(),
                        sql_example: rec.sql_example.clone(),
                        source: "llm".to_string(),
                        related_root_causes: vec![],
                        is_root_cause_fix: true,
                    })
            })
            .collect();

        let mut rule_priority = merged.len() as u32 + 1;
        rule_diagnostics
            .iter()
            .flat_map(|diag| std::iter::repeat(diag).zip(&diag.suggestions))
            .for_each(|(diag, suggestion)| {
                let action_key = Self::normalize_action(suggestion);
                if seen_actions.insert(action_key.clone()) {
                    merged.push(MergedRecommendation {
                        priority: rule_priority,
                        action: suggestion.clone(),
                        expected_improvement: String::new(),
                        sql_example: None,
                        source: "rule".to_string(),
                        related_root_causes: vec![diag.rule_id.clone()],
                        is_root_cause_fix: false,
                    });
                    rule_priority += 1;
                } else if let Some(existing) = merged
                    .iter_mut()
                    .find(|r| Self::normalize_action(&r.action) == action_key)
                {
                    if existing.source == "llm" {
                        existing.source = "both".to_string();
                    }
                    existing.related_root_causes.push(diag.rule_id.clone());
                }
            });

        merged.sort_by_key(|r| r.priority);

        merged
    }

    /// Normalize action text for deduplication
    fn normalize_action(action: &str) -> String {
        action
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }
}

// ============================================================================
// Conversion from Rule Diagnostics
// ============================================================================

/// Convert DiagnosticResult to DiagnosticForLLM
pub fn diagnostic_to_llm(diag: &DiagnosticResult) -> super::root_cause::DiagnosticForLLM {
    use std::collections::HashMap;

    let mut evidence = HashMap::new();
    evidence.insert("reason".to_string(), diag.reason.clone());

    // Convert threshold metadata if present
    let threshold_info =
        diag.threshold_metadata
            .as_ref()
            .map(|tm| super::root_cause::ThresholdInfoForLLM {
                threshold_value: tm.threshold_value,
                source: tm.threshold_source.clone(),
                baseline_p95_ms: tm.baseline_p95_ms,
                sample_count: tm.baseline_sample_count,
            });

    super::root_cause::DiagnosticForLLM {
        rule_id: diag.rule_id.clone(),
        severity: diag.severity.clone(),
        operator: diag
            .node_path
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .to_string(),
        plan_node_id: diag.plan_node_id,
        message: diag.message.clone(),
        evidence,
        threshold_info,
    }
}

/// Convert AggregatedDiagnostic to DiagnosticForLLM
pub fn aggregated_diagnostic_to_llm(
    diag: &AggregatedDiagnostic,
) -> super::root_cause::DiagnosticForLLM {
    use std::collections::HashMap;

    let mut evidence = HashMap::new();
    evidence.insert("reason".to_string(), diag.reason.clone());
    evidence.insert("affected_nodes".to_string(), format!("{} nodes", diag.node_count));

    super::root_cause::DiagnosticForLLM {
        rule_id: diag.rule_id.clone(),
        severity: diag.severity.clone(),
        operator: diag
            .affected_nodes
            .first()
            .map(|s| s.split('/').next_back().unwrap_or("unknown"))
            .unwrap_or("unknown")
            .to_string(),
        plan_node_id: None,
        message: diag.message.clone(),
        evidence,
        // AggregatedDiagnostic doesn't have threshold_metadata, so we set None
        threshold_info: None,
    }
}
