//! StarRocks Profile Analyzer
//! 
//! A comprehensive module for parsing, analyzing, and visualizing StarRocks query profiles.
//! 
//! # Architecture
//! 
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    ProfileAnalyzer                          │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │                   analyze_profile()                  │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! │                           │                                 │
//! │           ┌───────────────┼───────────────┐                │
//! │           ▼               ▼               ▼                │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
//! │  │   Parser    │  │  Analyzer   │  │   Models    │        │
//! │  │  ┌───────┐  │  │  ┌───────┐  │  │             │        │
//! │  │  │Composer│  │  │  │Hotspot│  │  │  Profile    │        │
//! │  │  └───────┘  │  │  │Detector│  │  │  Summary    │        │
//! │  │  ┌───────┐  │  │  └───────┘  │  │  ExecTree   │        │
//! │  │  │Section│  │  │  ┌───────┐  │  │  Fragment   │        │
//! │  │  │Parser │  │  │  │Suggest│  │  │  ...        │        │
//! │  │  └───────┘  │  │  │Engine │  │  │             │        │
//! │  │  ┌───────┐  │  │  └───────┘  │  │             │        │
//! │  │  │Topology│ │  │             │  │             │        │
//! │  │  │Parser │  │  │             │  │             │        │
//! │  │  └───────┘  │  │             │  │             │        │
//! │  └─────────────┘  └─────────────┘  └─────────────┘        │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//! 
//! # Usage
//! 
//! ```ignore
//! use starrocks_admin_backend::services::profile_analyzer::analyze_profile;
//! 
//! let profile_text = "..."; // Raw profile text from StarRocks
//! let result = analyze_profile(profile_text)?;
//! 
//! // Access parsed data
//! println!("Query ID: {}", result.summary.as_ref().unwrap().query_id);
//! println!("Performance Score: {}", result.performance_score);
//! ```

pub mod models;
pub mod parser;
pub mod analyzer;

#[cfg(test)]
mod tests;

pub use models::*;
pub use parser::ProfileComposer;
pub use analyzer::RuleEngine;

/// Analyze a profile text and return complete analysis results
/// 
/// This is the main entry point for profile analysis. It:
/// 1. Parses the profile text into structured data
/// 2. Builds the execution tree for DAG visualization
/// 3. Detects performance hotspots
/// 4. Generates optimization suggestions
/// 5. Calculates a performance score
/// 
/// # Arguments
/// 
/// * `profile_text` - Raw profile text from StarRocks (from `get_query_profile()` or `SHOW PROFILE`)
/// 
/// # Returns
/// 
/// * `Ok(ProfileAnalysisResponse)` - Complete analysis results
/// * `Err(String)` - Error message if parsing fails
/// 
/// # Example
/// 
/// ```ignore
/// let result = analyze_profile(profile_text)?;
/// 
/// // Check for hotspots
/// for hotspot in &result.hotspots {
///     println!("{}: {}", hotspot.node_path, hotspot.description);
/// }
/// 
/// // Access execution tree for visualization
/// if let Some(tree) = &result.execution_tree {
///     for node in &tree.nodes {
///         println!("{}: {:.2}%", node.operator_name, node.time_percentage.unwrap_or(0.0));
///     }
/// }
/// ```
pub fn analyze_profile(profile_text: &str) -> Result<ProfileAnalysisResponse, String> {
    use std::collections::HashMap;
    
    let mut composer = ProfileComposer::new();
    let profile = composer.parse(profile_text)
        .map_err(|e| format!("解析Profile失败: {:?}", e))?;
    
    let mut execution_tree = profile.execution_tree.clone();
    let summary = Some(profile.summary.clone());
    
    // Run RuleEngine for diagnostics
    let rule_engine = RuleEngine::new();
    let rule_diagnostics = rule_engine.analyze(&profile);
    
    // Convert rule diagnostics to API response format
    let diagnostics: Vec<DiagnosticResult> = rule_diagnostics.iter().map(|d| {
        DiagnosticResult {
            rule_id: d.rule_id.clone(),
            rule_name: d.rule_name.clone(),
            severity: format!("{:?}", d.severity),
            node_path: d.node_path.clone(),
            plan_node_id: d.plan_node_id,
            message: d.message.clone(),
            reason: d.reason.clone(),
            suggestions: d.suggestions.clone(),
            parameter_suggestions: d.parameter_suggestions.iter().map(|p| {
                ParameterTuningSuggestion {
                    name: p.name.clone(),
                    param_type: format!("{:?}", p.param_type),
                    current: p.current.clone(),
                    recommended: p.recommended.clone(),
                    command: p.command.clone(),
                }
            }).collect(),
        }
    }).collect();
    
    // Build node_diagnostics mapping (plan_node_id -> diagnostics)
    let mut node_diagnostics: HashMap<i32, Vec<DiagnosticResult>> = HashMap::new();
    for diag in &diagnostics {
        if let Some(plan_node_id) = diag.plan_node_id {
            node_diagnostics.entry(plan_node_id)
                .or_default()
                .push(diag.clone());
        }
    }
    
    // Update execution tree nodes with diagnostic info
    if let Some(ref mut tree) = execution_tree {
        for node in &mut tree.nodes {
            if let Some(plan_node_id) = node.plan_node_id {
                if let Some(node_diags) = node_diagnostics.get(&plan_node_id) {
                    node.has_diagnostic = true;
                    node.diagnostic_ids = node_diags.iter()
                        .map(|d| d.rule_id.clone())
                        .collect();
                }
            }
        }
    }
    
    // Aggregate diagnostics by rule_id for overview display
    let aggregated_diagnostics = aggregate_diagnostics(&diagnostics);
    
    // Generate hotspots from diagnostics for backward compatibility
    let hotspots: Vec<HotSpot> = rule_diagnostics.iter().map(|d| d.to_hotspot()).collect();
    
    // Generate conclusion, suggestions and performance score using RuleEngine
    let conclusion = RuleEngine::generate_conclusion(&rule_diagnostics, &profile);
    let all_suggestions = RuleEngine::generate_suggestions(&rule_diagnostics);
    let performance_score = RuleEngine::calculate_performance_score(&rule_diagnostics, &profile);
    
    Ok(ProfileAnalysisResponse {
        hotspots,
        conclusion,
        suggestions: all_suggestions,
        performance_score,
        execution_tree,
        summary,
        diagnostics,
        aggregated_diagnostics,
        node_diagnostics,
        profile_content: Some(profile_text.to_string()),
    })
}

/// Aggregate diagnostics by rule_id for overview display
/// Groups multiple diagnostics of the same rule together
fn aggregate_diagnostics(diagnostics: &[DiagnosticResult]) -> Vec<AggregatedDiagnostic> {
    use std::collections::{HashMap, HashSet};
    
    // Group by rule_id
    let mut groups: HashMap<String, Vec<&DiagnosticResult>> = HashMap::new();
    for diag in diagnostics {
        groups.entry(diag.rule_id.clone())
            .or_default()
            .push(diag);
    }
    
    // Convert groups to aggregated diagnostics
    let mut result: Vec<AggregatedDiagnostic> = groups.into_iter().map(|(rule_id, diags)| {
        let first = diags.first().unwrap();
        
        // Collect affected nodes
        let affected_nodes: Vec<String> = diags.iter()
            .map(|d| d.node_path.clone())
            .collect();
        
        // Merge and deduplicate suggestions
        let mut suggestions_set: HashSet<String> = HashSet::new();
        for diag in &diags {
            for suggestion in &diag.suggestions {
                suggestions_set.insert(suggestion.clone());
            }
        }
        let suggestions: Vec<String> = suggestions_set.into_iter().collect();
        
        // Merge parameter suggestions (take first non-empty)
        let parameter_suggestions = diags.iter()
            .find(|d| !d.parameter_suggestions.is_empty())
            .map(|d| d.parameter_suggestions.clone())
            .unwrap_or_default();
        
        // Determine highest severity
        let severity = diags.iter()
            .map(|d| &d.severity)
            .max_by(|a, b| severity_order(a).cmp(&severity_order(b)))
            .unwrap_or(&first.severity)
            .clone();
        
        // Generate aggregated message
        let node_count = affected_nodes.len();
        let message = if node_count > 1 {
            format!("{} 个节点存在此问题", node_count)
        } else {
            first.message.clone()
        };
        
        AggregatedDiagnostic {
            rule_id,
            rule_name: first.rule_name.clone(),
            severity,
            message,
            reason: first.reason.clone(),
            affected_nodes,
            node_count,
            suggestions,
            parameter_suggestions,
        }
    }).collect();
    
    // Sort by severity (Error > Warning > Info) then by node_count
    result.sort_by(|a, b| {
        let severity_cmp = severity_order(&b.severity).cmp(&severity_order(&a.severity));
        if severity_cmp != std::cmp::Ordering::Equal {
            severity_cmp
        } else {
            b.node_count.cmp(&a.node_count)
        }
    });
    
    result
}

/// Get severity order for sorting (higher = more severe)
fn severity_order(severity: &str) -> u8 {
    match severity {
        "Error" => 3,
        "Warning" => 2,
        "Info" => 1,
        _ => 0,
    }
}

