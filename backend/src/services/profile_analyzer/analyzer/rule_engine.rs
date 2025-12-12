//! Rule Engine for Profile Diagnostics
//!
//! Orchestrates rule evaluation, deduplication, suggestion generation,
//! conclusion and performance score calculation.

use super::baseline::QueryComplexity;
use super::rules::{
    Diagnostic, DiagnosticRule, RuleContext, RuleSeverity, get_all_rules, get_query_rules,
};
use super::thresholds::{DynamicThresholds, QueryType};
use crate::services::profile_analyzer::models::*;
use std::collections::HashSet;

/// Rule engine configuration
#[derive(Debug, Clone)]
pub struct RuleEngineConfig {
    /// Maximum number of suggestions to return
    pub max_suggestions: usize,
    /// Whether to include parameter suggestions
    pub include_parameters: bool,
    /// Minimum severity to report
    pub min_severity: RuleSeverity,
}

impl Default for RuleEngineConfig {
    fn default() -> Self {
        Self {
            max_suggestions: 100, // Increased from 5 to avoid truncating important diagnostics
            include_parameters: true,
            min_severity: RuleSeverity::Info,
        }
    }
}

/// Rule engine for profile diagnostics
pub struct RuleEngine {
    config: RuleEngineConfig,
    rules: Vec<Box<dyn DiagnosticRule>>,
}

impl RuleEngine {
    /// Create a new rule engine with default configuration
    pub fn new() -> Self {
        Self { config: RuleEngineConfig::default(), rules: get_all_rules() }
    }

    /// Create with custom configuration (used in tests)
    #[cfg(test)]
    pub fn with_config(config: RuleEngineConfig) -> Self {
        Self { config, rules: get_all_rules() }
    }

    /// Analyze a profile and return diagnostics (for backward compatibility and tests)
    #[allow(dead_code)]
    pub fn analyze(&self, profile: &Profile) -> Vec<Diagnostic> {
        self.analyze_with_cluster_variables(profile, None)
    }

    /// Analyze a profile with optional live cluster variables
    /// cluster_variables: actual current values from the cluster (takes precedence)
    pub fn analyze_with_cluster_variables(
        &self,
        profile: &Profile,
        cluster_variables: Option<&std::collections::HashMap<String, String>>,
    ) -> Vec<Diagnostic> {
        // Default cluster_id = 0 for backward compatibility
        self.analyze_with_baseline(profile, cluster_variables, None)
    }

    /// Analyze a profile with baseline support
    /// cluster_id: used to fetch historical baseline from BaselineProvider
    pub fn analyze_with_baseline(
        &self,
        profile: &Profile,
        cluster_variables: Option<&std::collections::HashMap<String, String>>,
        cluster_id: Option<i64>,
    ) -> Vec<Diagnostic> {
        // Detect query type from SQL for dynamic thresholds
        let query_type = QueryType::from_sql(&profile.summary.sql_statement);

        // Detect query complexity for adaptive thresholds
        let query_complexity = QueryComplexity::from_sql(&profile.summary.sql_statement);

        // Get cluster info for smart recommendations
        let cluster_info = profile.get_cluster_info();

        // Get historical baseline from cache (if cluster_id provided)
        let baseline = cluster_id.map(|cid| super::BaselineProvider::get(cid, query_complexity));

        // Create dynamic thresholds based on cluster info, query type, complexity, and baseline
        let thresholds = if let Some(bl) = baseline {
            DynamicThresholds::with_baseline(cluster_info.clone(), query_type, query_complexity, bl)
        } else {
            DynamicThresholds::new(cluster_info.clone(), query_type, query_complexity)
        };

        // P0.1: Skip diagnosis for fast queries
        // v2.0: Use dynamic threshold based on query type (ETL allows faster queries to be diagnosed)
        let min_diagnosis_time = thresholds.get_min_diagnosis_time_seconds();
        if let Ok(total_time_seconds) = Self::parse_total_time(&profile.summary.total_time)
            && total_time_seconds < min_diagnosis_time {
                // Fast query - return empty diagnostics
                return vec![];
        }

        let mut diagnostics = Vec::new();

        // Evaluate query-level rules first
        let query_ctx = super::rules::query::QueryRuleContext::new(
            profile,
            cluster_variables,
            thresholds.clone(),
        );
        for rule in get_query_rules() {
            if let Some(diag) = rule.evaluate(&query_ctx)
                && diag.severity >= self.config.min_severity
            {
                diagnostics.push(Diagnostic {
                    rule_id: diag.rule_id,
                    rule_name: diag.rule_name,
                    severity: diag.severity,
                    node_path: "Query".to_string(),
                    plan_node_id: None,
                    message: diag.message,
                    reason: diag.reason,
                    suggestions: diag.suggestions,
                    parameter_suggestions: if self.config.include_parameters {
                        diag.parameter_suggestions
                    } else {
                        vec![]
                    },
                    // Pass through threshold metadata from QueryDiagnostic
                    threshold_metadata: diag.threshold_metadata,
                });
            }
        }

        // Evaluate planner-level rules (HMS metadata, optimizer, etc.)
        let query_time_ms = Self::parse_total_time(&profile.summary.total_time)
            .map(|s| s * 1000.0)
            .unwrap_or(0.0);
        let planner_ctx =
            super::rules::planner::PlannerRuleContext { planner: &profile.planner, query_time_ms };
        for rule in super::rules::planner::get_rules() {
            if let Some(mut diag) = rule.evaluate(&planner_ctx)
                && diag.severity >= self.config.min_severity
            {
                if !self.config.include_parameters {
                    diag.parameter_suggestions.clear();
                }
                diagnostics.push(diag);
            }
        }

        // Evaluate node-level rules
        if let Some(execution_tree) = &profile.execution_tree {
            // Get session variables for context
            let session_variables = &profile.summary.non_default_variables;
            // Get default database for table name resolution
            let default_db = profile.summary.default_db.as_deref();

            for node in &execution_tree.nodes {
                let context = RuleContext {
                    node,
                    session_variables,
                    cluster_info: Some(cluster_info.clone()),
                    cluster_variables,
                    default_db,
                    thresholds: thresholds.clone(),
                };

                for rule in &self.rules {
                    // Skip rules based on query type
                    if query_type.should_skip_rule(rule.id()) {
                        continue;
                    }

                    if rule.applicable_to(node)
                        && let Some(mut diag) = rule.evaluate(&context)
                        && diag.severity >= self.config.min_severity
                    {
                        if !self.config.include_parameters {
                            diag.parameter_suggestions.clear();
                        }
                        diagnostics.push(diag);
                    }
                }
            }
        }

        // P3: Performance regression detection using query history
        if let Some(regression) = super::query_history::QUERY_HISTORY.record_and_detect(profile) {
            diagnostics.push(regression);
        }

        // Sort by severity (highest first)
        diagnostics.sort_by(|a, b| b.severity.cmp(&a.severity));

        // Deduplicate similar diagnostics
        diagnostics = self.deduplicate(diagnostics);

        // Limit results
        if diagnostics.len() > self.config.max_suggestions {
            diagnostics.truncate(self.config.max_suggestions);
        }

        diagnostics
    }

    /// Deduplicate diagnostics by rule_id and node
    fn deduplicate(&self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for diag in diagnostics {
            let key = format!("{}:{}", diag.rule_id, diag.node_path);
            if seen.insert(key) {
                result.push(diag);
            }
        }

        result
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Conclusion and Score Generation (moved from SuggestionEngine)
// ============================================================================

impl RuleEngine {
    /// Generate a conclusion based on diagnostics and profile
    pub fn generate_conclusion(diagnostics: &[Diagnostic], profile: &Profile) -> String {
        if diagnostics.is_empty() {
            return "查询执行良好，未发现明显性能问题。".to_string();
        }

        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == RuleSeverity::Error)
            .count();
        let warning_count = diagnostics
            .iter()
            .filter(|d| d.severity == RuleSeverity::Warning)
            .count();

        let total_time = Self::parse_total_time(&profile.summary.total_time).unwrap_or(0.0);

        if error_count > 0 {
            format!(
                "查询存在{}个严重性能问题，执行时间较长（{}）。主要问题是{}。建议优先解决严重问题。",
                error_count,
                Self::format_duration(total_time),
                diagnostics
                    .first()
                    .map(|d| d.rule_name.as_str())
                    .unwrap_or("未知")
            )
        } else if warning_count > 2 {
            format!(
                "查询存在{}个中等程度性能问题，整体性能需优化。执行时间{}。",
                warning_count,
                Self::format_duration(total_time)
            )
        } else if total_time > 300.0 {
            format!("查询执行时间较长（{}），建议关注性能热点。", Self::format_duration(total_time))
        } else {
            format!("查询发现{}个小问题，整体性能可接受。", diagnostics.len())
        }
    }

    /// Generate aggregated suggestions from diagnostics
    pub fn generate_suggestions(diagnostics: &[Diagnostic]) -> Vec<String> {
        let mut suggestions = Vec::new();
        let mut unique_suggestions = HashSet::new();

        // Collect unique suggestions from diagnostics
        for diag in diagnostics {
            for suggestion in &diag.suggestions {
                if unique_suggestions.insert(suggestion.clone()) {
                    suggestions.push(suggestion.clone());
                }
            }
        }

        suggestions
    }

    /// Calculate performance score (0-100) based on diagnostics
    pub fn calculate_performance_score(diagnostics: &[Diagnostic], profile: &Profile) -> f64 {
        let mut score: f64 = 100.0;

        // Deduct points for diagnostics based on severity
        for diag in diagnostics {
            let penalty = match diag.severity {
                RuleSeverity::Error => 20.0,
                RuleSeverity::Warning => 10.0,
                RuleSeverity::Info => 3.0,
            };
            score -= penalty;
        }

        // Deduct points for long execution time
        if let Ok(total_seconds) = Self::parse_total_time(&profile.summary.total_time) {
            if total_seconds > 3600.0 {
                score -= 20.0;
            } else if total_seconds > 1800.0 {
                score -= 10.0;
            } else if total_seconds > 300.0 {
                score -= 5.0;
            }
        }

        score.max(0.0)
    }

    /// Parse total time string to seconds
    fn parse_total_time(time_str: &str) -> Result<f64, ()> {
        let s = time_str.trim();
        if s.is_empty() {
            return Err(());
        }

        let mut total_seconds = 0.0;
        let mut num_buf = String::new();
        let mut found_unit = false;
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() || c == '.' {
                num_buf.push(c);
                i += 1;
            } else {
                let value: f64 = num_buf.parse().unwrap_or(0.0);
                num_buf.clear();

                if c == 'h' {
                    total_seconds += value * 3600.0;
                    found_unit = true;
                    i += 1;
                } else if c == 'm' {
                    if i + 1 < chars.len() && chars[i + 1] == 's' {
                        total_seconds += value / 1000.0;
                        i += 2;
                    } else {
                        total_seconds += value * 60.0;
                        i += 1;
                    }
                    found_unit = true;
                } else if c == 's' {
                    total_seconds += value;
                    found_unit = true;
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }

        if found_unit { Ok(total_seconds) } else { Err(()) }
    }

    /// Format duration to human-readable string
    fn format_duration(seconds: f64) -> String {
        if seconds >= 3600.0 {
            format!("{:.1}小时", seconds / 3600.0)
        } else if seconds >= 60.0 {
            format!("{:.0}分钟", seconds / 60.0)
        } else {
            format!("{:.1}秒", seconds)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_engine_creation() {
        let engine = RuleEngine::new();
        assert!(!engine.rules.is_empty());
    }
}
