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
    let mut composer = ProfileComposer::new();
    let profile = composer.parse(profile_text)
        .map_err(|e| format!("解析Profile失败: {:?}", e))?;
    
    let execution_tree = profile.execution_tree.clone();
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
            message: d.message.clone(),
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
        profile_content: Some(profile_text.to_string()),
    })
}

