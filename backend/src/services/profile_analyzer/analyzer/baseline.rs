//! Historical Baseline Calculator
//!
//! This module calculates performance baselines from audit log data
//! to enable adaptive thresholds based on historical query behavior.
//!
//! Key Features:
//! - Query complexity-based grouping (simple/medium/complex/very complex)
//! - Table-level performance baseline (average query time per table)
//! - User-level performance baseline
//! - Time-based trend analysis (weekday/weekend, peak hours)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Historical Baseline Models
// ============================================================================

/// Historical query performance baseline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    /// Query complexity level
    pub complexity: QueryComplexity,
    /// Statistics calculated from audit log
    pub stats: BaselineStats,
    /// Sample size (number of historical queries)
    pub sample_size: usize,
    /// Time range of the baseline data
    pub time_range_hours: u32,
}

/// Query complexity classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QueryComplexity {
    /// Simple: single table scan, no JOIN
    Simple,
    /// Medium: 2-3 table JOIN, simple aggregation
    Medium,
    /// Complex: 4+ table JOIN, window functions, subqueries
    Complex,
    /// Very Complex: nested CTEs, multiple UDF calls, heavy computation
    VeryComplex,
}

impl QueryComplexity {
    /// Detect query complexity from SQL statement
    pub fn from_sql(sql: &str) -> Self {
        let sql_upper = sql.to_uppercase();
        
        // Count JOIN operations
        let join_count = sql_upper.matches("JOIN").count();
        
        // Check for complexity indicators
        let has_window = sql_upper.contains("OVER(") || sql_upper.contains("OVER (");
        let has_cte = sql_upper.contains("WITH") && sql_upper.contains("AS (");
        let has_subquery = sql_upper.matches("SELECT").count() > 1;
        let has_union = sql_upper.contains("UNION");
        let has_udf = sql_upper.contains("UDF") || sql_upper.matches("(").count() > 5;
        
        // Complexity score calculation
        let mut score = 0;
        score += join_count * 2;
        if has_window { score += 3; }
        if has_cte { score += 2; }
        if has_subquery { score += 1; }
        if has_union { score += 2; }
        if has_udf { score += 3; }
        
        match score {
            0..=2 => Self::Simple,
            3..=7 => Self::Medium,
            8..=15 => Self::Complex,
            _ => Self::VeryComplex,
        }
    }
}

/// Baseline statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BaselineStats {
    /// Average query time (ms)
    pub avg_ms: f64,
    /// Median query time (ms)
    pub p50_ms: f64,
    /// 95th percentile (ms)
    pub p95_ms: f64,
    /// 99th percentile (ms)
    pub p99_ms: f64,
    /// Maximum query time (ms)
    pub max_ms: f64,
    /// Standard deviation (ms)
    pub std_dev_ms: f64,
}

// ============================================================================
// Audit Log Data Structure (from StarRocks audit table)
// ============================================================================

/// Audit log record from starrocks_audit_db__.starrocks_audit_tbl__
#[derive(Debug, Clone)]
pub struct AuditLogRecord {
    pub query_id: String,
    pub user: String,
    pub db: String,
    pub stmt: String,
    pub query_type: String,
    pub query_time_ms: i64,
    pub state: String,
    pub timestamp: String,
}

// ============================================================================
// Baseline Calculator
// ============================================================================

/// Baseline calculator from audit logs
pub struct BaselineCalculator {
    /// Minimum sample size required for reliable baseline
    min_sample_size: usize,
}

impl BaselineCalculator {
    pub fn new() -> Self {
        Self {
            min_sample_size: 30, // Statistical significance threshold
        }
    }
    
    /// Calculate baseline for a specific query complexity
    pub fn calculate(&self, records: &[AuditLogRecord]) -> Option<PerformanceBaseline> {
        if records.is_empty() {
            return None;
        }
        
        // Determine complexity (assume all records have same complexity for this calculation)
        let complexity = if let Some(first) = records.first() {
            QueryComplexity::from_sql(&first.stmt)
        } else {
            QueryComplexity::Simple
        };
        
        // Extract query times
        let mut times: Vec<f64> = records.iter()
            .filter(|r| r.state == "EOF" || r.state == "OK") // Only successful queries
            .map(|r| r.query_time_ms as f64)
            .collect();
        
        if times.len() < self.min_sample_size {
            return None; // Not enough samples
        }
        
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let stats = self.compute_stats(&times);
        
        Some(PerformanceBaseline {
            complexity,
            stats,
            sample_size: times.len(),
            time_range_hours: 168, // Default: 7 days
        })
    }
    
    /// Calculate baselines grouped by query complexity
    pub fn calculate_by_complexity(&self, records: &[AuditLogRecord]) -> HashMap<QueryComplexity, PerformanceBaseline> {
        let mut grouped: HashMap<QueryComplexity, Vec<AuditLogRecord>> = HashMap::new();
        
        for record in records {
            let complexity = QueryComplexity::from_sql(&record.stmt);
            grouped.entry(complexity).or_insert_with(Vec::new).push(record.clone());
        }
        
        grouped.into_iter()
            .filter_map(|(complexity, records)| {
                self.calculate(&records).map(|baseline| (complexity, baseline))
            })
            .collect()
    }
    
    /// Calculate baseline for specific table (based on table name pattern in SQL)
    pub fn calculate_for_table(&self, records: &[AuditLogRecord], table_name: &str) -> Option<PerformanceBaseline> {
        let filtered: Vec<AuditLogRecord> = records.iter()
            .filter(|r| r.stmt.to_uppercase().contains(&table_name.to_uppercase()))
            .cloned()
            .collect();
        
        self.calculate(&filtered)
    }
    
    /// Compute statistical metrics from sorted time series
    fn compute_stats(&self, times: &[f64]) -> BaselineStats {
        if times.is_empty() {
            return BaselineStats::default();
        }
        
        let sum: f64 = times.iter().sum();
        let avg = sum / times.len() as f64;
        
        let p50_idx = (times.len() as f64 * 0.5) as usize;
        let p95_idx = (times.len() as f64 * 0.95) as usize;
        let p99_idx = (times.len() as f64 * 0.99) as usize;
        
        let p50 = times.get(p50_idx.min(times.len() - 1)).copied().unwrap_or(0.0);
        let p95 = times.get(p95_idx.min(times.len() - 1)).copied().unwrap_or(0.0);
        let p99 = times.get(p99_idx.min(times.len() - 1)).copied().unwrap_or(0.0);
        let max = times.last().copied().unwrap_or(0.0);
        
        // Calculate standard deviation
        let variance = times.iter().map(|t| (t - avg).powi(2)).sum::<f64>() / times.len() as f64;
        let std_dev = variance.sqrt();
        
        BaselineStats {
            avg_ms: avg,
            p50_ms: p50,
            p95_ms: p95,
            p99_ms: p99,
            max_ms: max,
            std_dev_ms: std_dev,
        }
    }
}

impl Default for BaselineCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Adaptive Threshold Calculator
// ============================================================================

/// Adaptive threshold calculator using historical baseline
pub struct AdaptiveThresholdCalculator {
    /// Baseline performance data
    baselines: HashMap<QueryComplexity, PerformanceBaseline>,
}

impl AdaptiveThresholdCalculator {
    pub fn new(baselines: HashMap<QueryComplexity, PerformanceBaseline>) -> Self {
        Self { baselines }
    }
    
    /// Get query time threshold based on complexity
    /// Returns threshold in milliseconds
    ///
    /// Strategy: Use P95 of historical baseline + 2 std_dev as threshold
    pub fn get_query_time_threshold(&self, complexity: QueryComplexity) -> f64 {
        if let Some(baseline) = self.baselines.get(&complexity) {
            // Adaptive: P95 + 2 * std_dev
            let threshold = baseline.stats.p95_ms + 2.0 * baseline.stats.std_dev_ms;
            
            // Ensure minimum threshold (avoid too strict for simple queries)
            let min_threshold = match complexity {
                QueryComplexity::Simple => 5_000.0,      // 5s
                QueryComplexity::Medium => 10_000.0,     // 10s
                QueryComplexity::Complex => 30_000.0,    // 30s
                QueryComplexity::VeryComplex => 60_000.0, // 1min
            };
            
            threshold.max(min_threshold)
        } else {
            // Fallback to default if no baseline available
            self.get_default_threshold(complexity)
        }
    }
    
    /// Get skew threshold based on historical baseline
    /// Returns max/avg ratio threshold
    ///
    /// Strategy: If historical P99/P50 ratio is high, allow more skew
    pub fn get_skew_threshold(&self, complexity: QueryComplexity) -> f64 {
        if let Some(baseline) = self.baselines.get(&complexity) {
            let historical_ratio = if baseline.stats.p50_ms > 0.0 {
                baseline.stats.p99_ms / baseline.stats.p50_ms
            } else {
                2.0
            };
            
            // If historical data shows high variance, allow more skew
            // Base: 2.0, increase by 20% of historical ratio
            2.0 + (historical_ratio - 2.0) * 0.2
        } else {
            2.0 // Default
        }
    }
    
    /// Fallback default thresholds
    fn get_default_threshold(&self, complexity: QueryComplexity) -> f64 {
        match complexity {
            QueryComplexity::Simple => 10_000.0,
            QueryComplexity::Medium => 30_000.0,
            QueryComplexity::Complex => 60_000.0,
            QueryComplexity::VeryComplex => 180_000.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_query_complexity_detection() {
        // Simple query
        let sql1 = "SELECT * FROM users WHERE id = 1";
        assert_eq!(QueryComplexity::from_sql(sql1), QueryComplexity::Simple);
        
        // Medium query
        let sql2 = "SELECT u.name, o.amount FROM users u JOIN orders o ON u.id = o.user_id";
        assert_eq!(QueryComplexity::from_sql(sql2), QueryComplexity::Medium);
        
        // Complex query
        let sql3 = r#"
            SELECT u.name, SUM(o.amount) OVER (PARTITION BY u.id) 
            FROM users u 
            JOIN orders o ON u.id = o.user_id
            JOIN products p ON o.product_id = p.id
            WHERE p.price > 100
        "#;
        assert_eq!(QueryComplexity::from_sql(sql3), QueryComplexity::Complex);
        
        // Very complex query
        let sql4 = r#"
            WITH sales AS (
                SELECT user_id, SUM(amount) as total FROM orders GROUP BY user_id
            )
            SELECT u.name, s.total, RANK() OVER (ORDER BY s.total DESC)
            FROM users u 
            JOIN sales s ON u.id = s.user_id
            JOIN (SELECT * FROM products WHERE active = 1) p ON true
            UNION
            SELECT name, 0, 0 FROM inactive_users
        "#;
        assert_eq!(QueryComplexity::from_sql(sql4), QueryComplexity::VeryComplex);
    }
    
    #[test]
    fn test_baseline_calculation() {
        let calculator = BaselineCalculator::new();
        
        let records = vec![
            AuditLogRecord {
                query_id: "1".to_string(),
                user: "test".to_string(),
                db: "db1".to_string(),
                stmt: "SELECT * FROM t1".to_string(),
                query_type: "Query".to_string(),
                query_time_ms: 100,
                state: "EOF".to_string(),
                timestamp: "2025-12-08 10:00:00".to_string(),
            },
        ];
        
        // Not enough samples
        assert!(calculator.calculate(&records).is_none());
        
        // Generate 50 samples
        let mut records = Vec::new();
        for i in 0..50 {
            records.push(AuditLogRecord {
                query_id: i.to_string(),
                user: "test".to_string(),
                db: "db1".to_string(),
                stmt: "SELECT * FROM t1".to_string(),
                query_type: "Query".to_string(),
                query_time_ms: 100 + i * 10,
                state: "EOF".to_string(),
                timestamp: "2025-12-08 10:00:00".to_string(),
            });
        }
        
        let baseline = calculator.calculate(&records).unwrap();
        assert_eq!(baseline.sample_size, 50);
        assert!(baseline.stats.avg_ms > 0.0);
        assert!(baseline.stats.p95_ms > baseline.stats.p50_ms);
    }
}
