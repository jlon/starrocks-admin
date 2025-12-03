//! Section parser for StarRocks profile
//! 
//! Parses Summary, Planner, and Execution sections from profile text.

use crate::services::profile_analyzer::models::{ProfileSummary, PlannerInfo, ExecutionInfo};
use crate::services::profile_analyzer::parser::error::{ParseError, ParseResult};
use crate::services::profile_analyzer::parser::core::ValueParser;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

static SUMMARY_LINE_REGEX: Lazy<Regex> = 
    Lazy::new(|| Regex::new(r"^\s*-\s+([^:]+):\s*(.*)$").unwrap());

/// Parser for profile sections (Summary, Planner, Execution)
pub struct SectionParser;

impl SectionParser {
    /// Parse Summary section from profile text
    pub fn parse_summary(text: &str) -> ParseResult<ProfileSummary> {
        let summary_block = Self::extract_block(text, "Summary:")?;
        
        let mut fields = HashMap::new();
        for line in summary_block.lines() {
            if let Some(cap) = SUMMARY_LINE_REGEX.captures(line) {
                let key = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
                let value = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                fields.insert(key.to_string(), value.to_string());
            }
        }
        
        Ok(ProfileSummary {
            query_id: fields.get("Query ID").cloned().unwrap_or_default(),
            start_time: fields.get("Start Time").cloned().unwrap_or_default(),
            end_time: fields.get("End Time").cloned().unwrap_or_default(),
            total_time: fields.get("Total").cloned().unwrap_or_default(),
            query_state: fields.get("Query State").cloned().unwrap_or_default(),
            starrocks_version: fields.get("StarRocks Version").cloned().unwrap_or_default(),
            sql_statement: fields.get("Sql Statement").cloned().unwrap_or_default(),
            query_type: fields.get("Query Type").cloned(),
            user: fields.get("User").cloned(),
            default_db: fields.get("Default Db").cloned(),
            variables: HashMap::new(),
            query_allocated_memory: None,
            query_peak_memory: None,
            total_time_ms: Self::parse_total_time_ms(&fields.get("Total").cloned().unwrap_or_default()),
            query_cumulative_operator_time: fields.get("QueryCumulativeOperatorTime").cloned(),
            query_cumulative_operator_time_ms: fields.get("QueryCumulativeOperatorTime")
                .and_then(|time_str| Self::parse_total_time_ms(time_str)),
            query_execution_wall_time: fields.get("QueryExecutionWallTime").cloned(),
            query_execution_wall_time_ms: fields.get("QueryExecutionWallTime")
                .and_then(|time_str| Self::parse_total_time_ms(time_str)),
            
            // Execution time metrics (will be filled from Execution section)
            query_cumulative_cpu_time: None,
            query_cumulative_cpu_time_ms: None,
            query_cumulative_scan_time: None,
            query_cumulative_scan_time_ms: None,
            query_cumulative_network_time: None,
            query_cumulative_network_time_ms: None,
            query_peak_schedule_time: None,
            query_peak_schedule_time_ms: None,
            result_deliver_time: None,
            result_deliver_time_ms: None,
            
            // Memory metrics
            query_sum_memory_usage: None,
            query_deallocated_memory_usage: None,
            
            // Spill metrics
            query_spill_bytes: None,
            
            top_time_consuming_nodes: None,
        })
    }
    
    /// Parse Planner section from profile text
    pub fn parse_planner(text: &str) -> ParseResult<PlannerInfo> {
        let planner_block = Self::extract_block(text, "Planner:")?;
        let mut details = HashMap::new();
        
        for line in planner_block.lines() {
            if let Some(cap) = SUMMARY_LINE_REGEX.captures(line) {
                let key = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
                let value = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                details.insert(key.to_string(), value.to_string());
            }
        }
        
        Ok(PlannerInfo { details })
    }
    
    /// Parse Execution section from profile text
    pub fn parse_execution(text: &str) -> ParseResult<ExecutionInfo> {
        let execution_block = Self::extract_block(text, "Execution:")?;
        
        let topology = Self::extract_topology(&execution_block)?;
        
        let mut metrics = HashMap::new();
        for line in execution_block.lines() {
            if let Some(cap) = SUMMARY_LINE_REGEX.captures(line) {
                let key = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
                let value = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
                if !key.is_empty() && !value.is_empty() && key != "Topology" {
                    metrics.insert(key.to_string(), value.to_string());
                }
            }
        }
        
        Ok(ExecutionInfo { topology, metrics })
    }
    
    /// Extract a block of text for a given section marker
    fn extract_block(text: &str, section_marker: &str) -> ParseResult<String> {
        if let Some(start) = text.find(section_marker) {
            let before_marker = &text[..start];
            let marker_line_start = before_marker.rfind('\n').map(|pos| pos + 1).unwrap_or(0);
            let marker_line = &text[marker_line_start..start + section_marker.len()];
            let marker_indent = Self::get_indent(marker_line);
            
            let rest = &text[start + section_marker.len()..];
            let lines: Vec<&str> = rest.lines().collect();
            
            // Find end of section (next section at same or lower indent level)
            let mut end_pos = rest.len();
            for (i, line) in lines.iter().enumerate().skip(1) {
                if !line.trim().is_empty() {
                    let curr_indent = Self::get_indent(line);
                    // New section at same or lower indent level
                    if curr_indent <= marker_indent && line.trim().ends_with(':') {
                        let mut char_count = 0;
                        for j in 0..i {
                            char_count += lines[j].len() + 1;
                        }
                        end_pos = char_count;
                        break;
                    }
                }
            }
            
            Ok(rest[..end_pos].to_string())
        } else {
            Err(ParseError::SectionNotFound(section_marker.to_string()))
        }
    }
    
    /// Extract topology JSON from execution block
    fn extract_topology(text: &str) -> ParseResult<String> {
        if let Some(start_pos) = text.find("- Topology:") {
            let after_label = &text[start_pos + 11..];
            if let Some(json_start) = after_label.find('{') {
                let json_part = &after_label[json_start..];
                
                // Find matching closing brace
                let mut depth = 0;
                let mut end_pos = 0;
                
                for (i, ch) in json_part.char_indices() {
                    match ch {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end_pos = i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                
                if end_pos > 0 {
                    return Ok(json_part[..end_pos].to_string());
                }
            }
        }
        
        Ok(String::new())
    }
    
    /// Get indentation level of a line
    fn get_indent(line: &str) -> usize {
        line.chars().take_while(|c| c.is_whitespace()).count()
    }
    
    /// Parse total time string to milliseconds
    fn parse_total_time_ms(time_str: &str) -> Option<f64> {
        ValueParser::parse_time_to_ms(time_str).ok()
    }
    
    /// Extract execution metrics and update summary
    pub fn extract_execution_metrics(execution_info: &ExecutionInfo, summary: &mut ProfileSummary) {
        // Memory metrics
        if let Some(val) = execution_info.metrics.get("QueryAllocatedMemoryUsage") {
            summary.query_allocated_memory = ValueParser::parse_bytes(val).ok();
        }
        if let Some(val) = execution_info.metrics.get("QueryPeakMemoryUsagePerNode") {
            summary.query_peak_memory = ValueParser::parse_bytes(val).ok();
        }
        if let Some(val) = execution_info.metrics.get("QuerySumMemoryUsage") {
            summary.query_sum_memory_usage = Some(val.clone());
        }
        if let Some(val) = execution_info.metrics.get("QueryDeallocatedMemoryUsage") {
            summary.query_deallocated_memory_usage = Some(val.clone());
        }
        
        // Time metrics
        if let Some(val) = execution_info.metrics.get("QueryCumulativeCpuTime") {
            summary.query_cumulative_cpu_time = Some(val.clone());
            summary.query_cumulative_cpu_time_ms = ValueParser::parse_time_to_ms(val).ok();
        }
        if let Some(val) = execution_info.metrics.get("QueryCumulativeScanTime") {
            summary.query_cumulative_scan_time = Some(val.clone());
            summary.query_cumulative_scan_time_ms = ValueParser::parse_time_to_ms(val).ok();
        }
        if let Some(val) = execution_info.metrics.get("QueryCumulativeNetworkTime") {
            summary.query_cumulative_network_time = Some(val.clone());
            summary.query_cumulative_network_time_ms = ValueParser::parse_time_to_ms(val).ok();
        }
        if let Some(val) = execution_info.metrics.get("QueryPeakScheduleTime") {
            summary.query_peak_schedule_time = Some(val.clone());
            summary.query_peak_schedule_time_ms = ValueParser::parse_time_to_ms(val).ok();
        }
        if let Some(val) = execution_info.metrics.get("ResultDeliverTime") {
            summary.result_deliver_time = Some(val.clone());
            summary.result_deliver_time_ms = ValueParser::parse_time_to_ms(val).ok();
        }
        
        // Spill metrics
        if let Some(val) = execution_info.metrics.get("QuerySpillBytes") {
            summary.query_spill_bytes = Some(val.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_summary() {
        let profile = r#"
Query:
  Summary:
     - Query ID: b1f9a935-a967-11f0-b3d8-f69e292b7593
     - Start Time: 2025-10-15 09:38:48
     - Total: 1h30m
     - Query State: Finished
"#;
        let summary = SectionParser::parse_summary(profile).unwrap();
        assert_eq!(summary.query_id, "b1f9a935-a967-11f0-b3d8-f69e292b7593");
        assert_eq!(summary.total_time, "1h30m");
    }
}
