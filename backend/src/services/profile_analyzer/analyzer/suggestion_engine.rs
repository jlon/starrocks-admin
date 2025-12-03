//! Suggestion engine for profile analysis
//! 
//! Generates optimization suggestions and performance conclusions.

use crate::services::profile_analyzer::models::*;

/// Suggestion engine for generating optimization recommendations
pub struct SuggestionEngine;

impl SuggestionEngine {
    /// Generate a conclusion based on hotspots and profile
    pub fn generate_conclusion(hotspots: &[HotSpot], profile: &Profile) -> String {
        if hotspots.is_empty() {
            return "查询执行良好，未发现明显性能问题。".to_string();
        }
        
        let severe_count = hotspots.iter()
            .filter(|h| matches!(h.severity, HotSeverity::Severe | HotSeverity::Critical))
            .count();
        let moderate_count = hotspots.iter()
            .filter(|h| matches!(h.severity, HotSeverity::Moderate))
            .count();
        
        let total_time = Self::parse_total_time(&profile.summary.total_time).unwrap_or(0.0);
        
        if severe_count > 0 {
            format!(
                "查询存在{}个严重性能问题，执行时间较长（{}）。主要问题是{}。建议优先解决严重问题。",
                severe_count,
                Self::format_duration(total_time),
                hotspots.first().map(|h| h.issue_type.as_str()).unwrap_or("未知")
            )
        } else if moderate_count > 2 {
            format!(
                "查询存在{}个中等程度性能问题，整体性能需优化。执行时间{}。",
                moderate_count,
                Self::format_duration(total_time)
            )
        } else if total_time > 300.0 {
            format!("查询执行时间较长（{}），建议关注性能热点。", Self::format_duration(total_time))
        } else {
            format!("查询发现{}个小问题，整体性能可接受。", hotspots.len())
        }
    }
    
    /// Generate aggregated suggestions from hotspots
    pub fn generate_suggestions(hotspots: &[HotSpot]) -> Vec<String> {
        let mut suggestions = Vec::new();
        let mut unique_suggestions = std::collections::HashSet::new();
        
        // Collect unique suggestions from hotspots
        for hotspot in hotspots {
            for suggestion in &hotspot.suggestions {
                if unique_suggestions.insert(suggestion.clone()) {
                    suggestions.push(suggestion.clone());
                }
            }
        }
        
        // Add general suggestions
        let general_suggestions = vec![
            "考虑启用查询缓存以提高重复查询的性能".to_string(),
            "检查硬件资源（CPU、内存、存储）是否充足".to_string(),
            "定期维护表统计信息以优化查询计划".to_string(),
        ];
        
        for suggestion in general_suggestions {
            if unique_suggestions.insert(suggestion.clone()) {
                suggestions.push(suggestion);
            }
        }
        
        suggestions
    }
    
    /// Calculate performance score (0-100)
    pub fn calculate_performance_score(hotspots: &[HotSpot], profile: &Profile) -> f64 {
        let mut score: f64 = 100.0;
        
        // Deduct points for hotspots
        for hotspot in hotspots {
            let penalty = match hotspot.severity {
                HotSeverity::Critical => 25.0,
                HotSeverity::Severe => 15.0,
                HotSeverity::High => 12.0,
                HotSeverity::Moderate => 8.0,
                HotSeverity::Mild => 3.0,
                HotSeverity::Normal => 0.0,
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
    
    /// Parse total time string to seconds, supporting combined formats like "1h30m5s", "1m30s", "1s30ms"
    fn parse_total_time(time_str: &str) -> Result<f64, ()> {
        let s = time_str.trim();
        if s.is_empty() { return Err(()); }
        
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
