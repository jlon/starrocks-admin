//! Diagnostic rules module
//!
//! Implements the rule engine for Query Profile diagnostics.
//! Rules are organized by operator type following the design document.

pub mod common;
pub mod scan;
pub mod join;
pub mod aggregate;
pub mod sort;
pub mod exchange;
pub mod query;
pub mod fragment;
pub mod project;
pub mod sink;

use crate::services::profile_analyzer::models::*;

// ============================================================================
// Rule Trait and Types
// ============================================================================

/// Severity level for diagnostic rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
}

impl From<RuleSeverity> for HotSeverity {
    fn from(severity: RuleSeverity) -> Self {
        match severity {
            RuleSeverity::Info => HotSeverity::Mild,
            RuleSeverity::Warning => HotSeverity::Moderate,
            RuleSeverity::Error => HotSeverity::Severe,
        }
    }
}

/// Parameter suggestion for tuning
#[derive(Debug, Clone)]
pub struct ParameterSuggestion {
    pub name: String,
    pub param_type: ParameterType,
    pub current: Option<String>,
    pub recommended: String,
    pub command: String,
}

/// Parameter type classification
#[derive(Debug, Clone, Copy)]
pub enum ParameterType {
    Session,
    BE,
}

/// A diagnostic result from rule evaluation
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: RuleSeverity,
    pub node_path: String,
    pub message: String,
    pub suggestions: Vec<String>,
    pub parameter_suggestions: Vec<ParameterSuggestion>,
}

impl Diagnostic {
    /// Convert to HotSpot for backward compatibility
    pub fn to_hotspot(&self) -> HotSpot {
        let mut all_suggestions = self.suggestions.clone();
        
        // Add parameter suggestions as formatted strings
        for param in &self.parameter_suggestions {
            all_suggestions.push(format!(
                "调整参数: {} → {} (命令: {})",
                param.name, param.recommended, param.command
            ));
        }
        
        HotSpot {
            node_path: self.node_path.clone(),
            severity: self.severity.into(),
            issue_type: self.rule_id.clone(),
            description: self.message.clone(),
            suggestions: all_suggestions,
        }
    }
}

/// Context for rule evaluation
pub struct RuleContext<'a> {
    pub node: &'a ExecutionTreeNode,
}

impl<'a> RuleContext<'a> {
    /// Get a metric value from unique_metrics as f64
    pub fn get_metric(&self, name: &str) -> Option<f64> {
        self.node.unique_metrics.get(name)
            .and_then(|v| parse_metric_value(v))
    }
    
    /// Get operator total time in ms
    pub fn get_operator_time_ms(&self) -> Option<f64> {
        self.node.metrics.operator_total_time.map(|ns| ns as f64 / 1_000_000.0)
    }
    
    /// Get time percentage
    pub fn get_time_percentage(&self) -> Option<f64> {
        self.node.time_percentage
    }
    
    /// Get memory usage in bytes
    pub fn get_memory_usage(&self) -> Option<u64> {
        self.node.metrics.memory_usage
    }
    
}

/// Trait for diagnostic rules
pub trait DiagnosticRule: Send + Sync {
    /// Rule ID (e.g., "S001", "J001")
    fn id(&self) -> &str;
    
    /// Rule name
    fn name(&self) -> &str;
    
    /// Check if rule applies to this node
    fn applicable_to(&self, node: &ExecutionTreeNode) -> bool;
    
    /// Evaluate the rule and return diagnostic if triggered
    fn evaluate(&self, context: &RuleContext) -> Option<Diagnostic>;
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Parse metric value from string (handles various formats)
pub fn parse_metric_value(value: &str) -> Option<f64> {
    let s = value.trim();
    
    // Handle percentage
    if s.ends_with('%') {
        return s.trim_end_matches('%').parse().ok();
    }
    
    // Handle bytes (e.g., "1.5 GB", "100 MB")
    if let Some(bytes) = parse_bytes(s) {
        return Some(bytes as f64);
    }
    
    // Handle time (e.g., "1s500ms", "100ms")
    if let Some(ms) = parse_duration_ms(s) {
        return Some(ms);
    }
    
    // Handle plain numbers with optional suffix
    let numeric_part: String = s.chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    
    numeric_part.parse().ok()
}

/// Parse bytes string to u64
pub fn parse_bytes(s: &str) -> Option<u64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split_whitespace().collect();
    
    if parts.len() != 2 {
        return None;
    }
    
    let value: f64 = parts[0].parse().ok()?;
    let unit = parts[1].to_uppercase();
    
    let multiplier = match unit.as_str() {
        "B" => 1u64,
        "KB" | "K" => 1024,
        "MB" | "M" => 1024 * 1024,
        "GB" | "G" => 1024 * 1024 * 1024,
        "TB" | "T" => 1024 * 1024 * 1024 * 1024,
        _ => return None,
    };
    
    Some((value * multiplier as f64) as u64)
}

/// Parse duration string to milliseconds
pub fn parse_duration_ms(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    
    let mut total_ms = 0.0;
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
                total_ms += value * 3600.0 * 1000.0;
                found_unit = true;
                i += 1;
            } else if c == 'm' {
                if i + 1 < chars.len() && chars[i + 1] == 's' {
                    total_ms += value;
                    i += 2;
                } else {
                    total_ms += value * 60.0 * 1000.0;
                    i += 1;
                }
                found_unit = true;
            } else if c == 's' {
                total_ms += value * 1000.0;
                found_unit = true;
                i += 1;
            } else if c == 'u' && i + 1 < chars.len() && chars[i + 1] == 's' {
                total_ms += value / 1000.0;
                found_unit = true;
                i += 2;
            } else if c == 'n' && i + 1 < chars.len() && chars[i + 1] == 's' {
                total_ms += value / 1_000_000.0;
                found_unit = true;
                i += 2;
            } else {
                i += 1;
            }
        }
    }
    
    if found_unit { Some(total_ms) } else { None }
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Format duration in ms to human-readable string
pub fn format_duration_ms(ms: f64) -> String {
    if ms < 1.0 {
        format!("{:.2}μs", ms * 1000.0)
    } else if ms < 1000.0 {
        format!("{:.2}ms", ms)
    } else if ms < 60000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else if ms < 3600000.0 {
        format!("{:.1}m", ms / 60000.0)
    } else {
        format!("{:.1}h", ms / 3600000.0)
    }
}

// ============================================================================
// Rule Registry
// ============================================================================

/// Get all registered rules
pub fn get_all_rules() -> Vec<Box<dyn DiagnosticRule>> {
    let mut rules: Vec<Box<dyn DiagnosticRule>> = Vec::new();
    
    // Common rules (G001, G002, G003)
    rules.extend(common::get_rules());
    
    // Scan rules (S001-S011)
    rules.extend(scan::get_rules());
    
    // Join rules (J001-J010)
    rules.extend(join::get_rules());
    
    // Aggregate rules (A001-A005)
    rules.extend(aggregate::get_rules());
    
    // Sort rules (T001-T005, W001)
    rules.extend(sort::get_rules());
    
    // Exchange rules (E001-E003)
    rules.extend(exchange::get_rules());
    
    // Fragment rules (F001-F003)
    rules.extend(fragment::get_rules());
    
    // Project/LocalExchange rules (P001, L001)
    rules.extend(project::get_rules());
    
    // OlapTableSink rules (I001-I003)
    rules.extend(sink::get_rules());
    
    // Query rules (Q001-Q009) - evaluated separately at query level
    
    rules
}

/// Get query-level rules
pub fn get_query_rules() -> Vec<Box<dyn query::QueryRule>> {
    query::get_rules()
}
