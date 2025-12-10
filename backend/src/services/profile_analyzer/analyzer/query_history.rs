//! Query History Service - In-Memory Cache for Performance Regression Detection
//!
//! # Status: P3 Optional Feature (可选功能)
//!
//! This module provides **real-time** query fingerprinting and performance regression
//! detection (REG001). It is complementary to the audit log baseline system.
//!
//! ## Design Decision: Why This Exists
//!
//! | Feature | Audit Log Baseline | Query History (This) |
//! |---------|-------------------|---------------------|
//! | Data Source | Persisted audit_log | In-memory LRU cache |
//! | Survives Restart | ✅ Yes | ❌ No |
//! | Granularity | QueryComplexity | SQL Fingerprint |
//! | Latency | ~seconds | ~microseconds |
//! | Use Case | Adaptive thresholds | Real-time regression |
//!
//! ## Recommendation
//!
//! - **Production**: Consider migrating REG001 to use audit log baselines
//!   for persistent cross-restart regression detection.
//! - **Current**: Enabled by default for real-time regression detection
//!   within a single service lifetime.
//!
//! ## Features
//! - Query fingerprinting (normalize SQL to identify similar queries)
//! - LRU cache for execution baselines (10K entries by default)
//! - Performance regression detection (REG001)
//! - Zero-latency on cache hit

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::rules::{Diagnostic, RuleSeverity};
use super::thresholds::QueryType;
use crate::services::profile_analyzer::models::Profile;

// ============================================================================
// Global Instance
// ============================================================================

/// Global query history service instance
///
/// Controlled by environment variable `QUERY_HISTORY_ENABLED`:
/// - `QUERY_HISTORY_ENABLED=true` (default): Enable REG001 regression detection
/// - `QUERY_HISTORY_ENABLED=false`: Disable regression detection
pub static QUERY_HISTORY: Lazy<QueryHistoryService> = Lazy::new(|| {
    let enabled = std::env::var("QUERY_HISTORY_ENABLED")
        .map(|v| v.to_lowercase() != "false" && v != "0")
        .unwrap_or(true); // Default: enabled

    QueryHistoryService::with_config(HistoryConfig { enabled, ..Default::default() })
});

// ============================================================================
// Configuration
// ============================================================================

/// History service configuration
#[derive(Debug, Clone)]
pub struct HistoryConfig {
    /// Whether in-memory query history is enabled
    /// Set to false to disable REG001 regression detection
    /// (Recommended: use audit log baseline instead for production)
    pub enabled: bool,
    /// Max entries in memory cache (LRU eviction)
    pub memory_cache_size: usize,
    /// Min execution time to record (skip fast queries)
    pub min_record_time_ms: f64,
    /// Min samples before regression detection
    pub min_samples_for_regression: u32,
    /// Regression detection ratio (current/p90 > this = regression)
    pub regression_ratio_threshold: f64,
    /// Severe regression ratio (triggers Error severity)
    pub severe_regression_ratio: f64,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enabled by default for real-time detection
            memory_cache_size: 10000,
            min_record_time_ms: 100.0,       // Skip queries < 100ms
            min_samples_for_regression: 5,   // Need at least 5 samples
            regression_ratio_threshold: 2.0, // 2x slower = regression
            severe_regression_ratio: 5.0,    // 5x slower = severe
        }
    }
}

// ============================================================================
// Query Fingerprint
// ============================================================================

/// Query fingerprint for identifying similar queries
#[derive(Debug, Clone)]
pub struct QueryFingerprint {
    /// Normalized SQL template (parameterized)
    pub sql_template: String,
    /// Tables involved in the query
    pub tables: Vec<String>,
    /// Query type (SELECT, INSERT, etc.)
    pub query_type: QueryType,
    /// Hash of the fingerprint (cached for performance)
    hash: u64,
}

impl QueryFingerprint {
    /// Create fingerprint from profile
    pub fn from_profile(profile: &Profile) -> Self {
        let sql = &profile.summary.sql_statement;
        let sql_template = Self::normalize_sql(sql);
        let tables = Self::extract_tables(sql);
        let query_type = QueryType::from_sql(sql);

        let hash = Self::compute_hash(&sql_template, &tables, &query_type);

        Self { sql_template, tables, query_type, hash }
    }

    /// Normalize SQL: replace literals with placeholders
    fn normalize_sql(sql: &str) -> String {
        let mut result = sql.to_uppercase();

        // Replace numeric literals (keep simple for MVP)
        result = regex::Regex::new(r"\b\d+\.?\d*\b")
            .map(|re| re.replace_all(&result, "?").to_string())
            .unwrap_or(result);

        // Replace string literals
        result = regex::Regex::new(r"'[^']*'")
            .map(|re| re.replace_all(&result, "?").to_string())
            .unwrap_or(result);

        // Normalize whitespace
        result = regex::Regex::new(r"\s+")
            .map(|re| re.replace_all(&result, " ").to_string())
            .unwrap_or(result);

        result.trim().to_string()
    }

    /// Extract table names from SQL (simplified)
    fn extract_tables(sql: &str) -> Vec<String> {
        let upper = sql.to_uppercase();
        let mut tables = Vec::new();

        // Simple regex for FROM/JOIN clauses
        if let Ok(re) = regex::Regex::new(r"(?:FROM|JOIN)\s+`?(\w+)`?") {
            for cap in re.captures_iter(&upper) {
                if let Some(m) = cap.get(1) {
                    tables.push(m.as_str().to_string());
                }
            }
        }

        tables.sort();
        tables.dedup();
        tables
    }

    /// Compute hash for the fingerprint
    fn compute_hash(sql_template: &str, tables: &[String], query_type: &QueryType) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        sql_template.hash(&mut hasher);
        tables.hash(&mut hasher);
        std::mem::discriminant(query_type).hash(&mut hasher);
        hasher.finish()
    }

    /// Get fingerprint hash
    pub fn hash(&self) -> u64 {
        self.hash
    }
}

impl PartialEq for QueryFingerprint {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for QueryFingerprint {}

impl Hash for QueryFingerprint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

// ============================================================================
// Execution Statistics
// ============================================================================

/// Time statistics for query executions
#[derive(Debug, Clone, Default)]
pub struct TimeStats {
    /// Samples (sorted for percentile calculation)
    samples: Vec<f64>,
    /// Sum for average calculation
    sum: f64,
}

impl TimeStats {
    /// Add a sample
    pub fn add_sample(&mut self, time_ms: f64) {
        self.samples.push(time_ms);
        self.sum += time_ms;
        self.samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    }

    /// Get sample count
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// Get average
    pub fn avg(&self) -> f64 {
        if self.samples.is_empty() { 0.0 } else { self.sum / self.samples.len() as f64 }
    }

    /// Get P50 (median)
    pub fn p50(&self) -> f64 {
        self.percentile(50)
    }

    /// Get P90
    pub fn p90(&self) -> f64 {
        self.percentile(90)
    }

    /// Get P99
    pub fn p99(&self) -> f64 {
        self.percentile(99)
    }

    /// Get percentile value
    fn percentile(&self, p: u8) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let idx = (self.samples.len() as f64 * p as f64 / 100.0).ceil() as usize;
        let idx = idx.min(self.samples.len()).saturating_sub(1);
        self.samples[idx]
    }
}

/// Execution baseline for a query fingerprint
#[derive(Debug, Clone)]
pub struct ExecutionBaseline {
    /// Query fingerprint
    pub fingerprint: QueryFingerprint,
    /// Time statistics
    pub time_stats: TimeStats,
    /// Last updated timestamp
    pub last_updated: Instant,
    /// First seen timestamp
    pub first_seen: Instant,
}

impl ExecutionBaseline {
    /// Create new baseline
    pub fn new(fingerprint: QueryFingerprint, initial_time_ms: f64) -> Self {
        let mut time_stats = TimeStats::default();
        time_stats.add_sample(initial_time_ms);
        let now = Instant::now();
        Self { fingerprint, time_stats, last_updated: now, first_seen: now }
    }

    /// Record a new execution
    pub fn record_execution(&mut self, time_ms: f64) {
        self.time_stats.add_sample(time_ms);
        self.last_updated = Instant::now();
    }
}

// ============================================================================
// Query History Service
// ============================================================================

/// Query history service with LRU cache
pub struct QueryHistoryService {
    /// Configuration
    config: HistoryConfig,
    /// Baseline cache (fingerprint_hash -> baseline)
    cache: Arc<RwLock<HashMap<u64, ExecutionBaseline>>>,
    /// Access order for LRU eviction
    access_order: Arc<RwLock<Vec<u64>>>,
}

impl QueryHistoryService {
    /// Create new service with default config
    pub fn new() -> Self {
        Self::with_config(HistoryConfig::default())
    }

    /// Create new service with custom config
    pub fn with_config(config: HistoryConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            access_order: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a query execution and detect regression
    /// Returns None if disabled or no regression detected
    pub fn record_and_detect(&self, profile: &Profile) -> Option<Diagnostic> {
        // Check if feature is enabled
        if !self.config.enabled {
            return None;
        }

        let time_ms = profile.summary.total_time_ms?;

        // Skip fast queries
        if time_ms < self.config.min_record_time_ms {
            return None;
        }

        let fingerprint = QueryFingerprint::from_profile(profile);

        // Try to detect regression first (before updating baseline)
        let regression = self.detect_regression(&fingerprint, time_ms);

        // Update baseline
        self.record_execution(fingerprint, time_ms);

        regression
    }

    /// Check if query history tracking is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Record an execution (update baseline)
    fn record_execution(&self, fingerprint: QueryFingerprint, time_ms: f64) {
        let hash = fingerprint.hash();

        // Update access order for LRU
        {
            let mut order = self.access_order.write().unwrap();
            order.retain(|&h| h != hash);
            order.push(hash);
        }

        // Update or create baseline
        {
            let mut cache = self.cache.write().unwrap();

            if let Some(baseline) = cache.get_mut(&hash) {
                baseline.record_execution(time_ms);
            } else {
                // Check if we need to evict
                if cache.len() >= self.config.memory_cache_size {
                    self.evict_lru(&mut cache);
                }
                cache.insert(hash, ExecutionBaseline::new(fingerprint, time_ms));
            }
        }
    }

    /// Evict least recently used entry
    fn evict_lru(&self, cache: &mut HashMap<u64, ExecutionBaseline>) {
        let mut order = self.access_order.write().unwrap();
        if let Some(oldest_hash) = order.first().copied() {
            cache.remove(&oldest_hash);
            order.remove(0);
        }
    }

    /// Detect performance regression
    fn detect_regression(
        &self,
        fingerprint: &QueryFingerprint,
        current_time_ms: f64,
    ) -> Option<Diagnostic> {
        let cache = self.cache.read().unwrap();
        let baseline = cache.get(&fingerprint.hash())?;

        // Need minimum samples
        if baseline.time_stats.count() < self.config.min_samples_for_regression as usize {
            return None;
        }

        let p90 = baseline.time_stats.p90();
        if p90 == 0.0 {
            return None;
        }

        let ratio = current_time_ms / p90;

        if ratio > self.config.regression_ratio_threshold {
            let severity = if ratio > self.config.severe_regression_ratio {
                RuleSeverity::Error
            } else {
                RuleSeverity::Warning
            };

            let time_display = if current_time_ms >= 60000.0 {
                format!("{:.1}分钟", current_time_ms / 60000.0)
            } else if current_time_ms >= 1000.0 {
                format!("{:.1}秒", current_time_ms / 1000.0)
            } else {
                format!("{:.0}ms", current_time_ms)
            };

            let p90_display = if p90 >= 60000.0 {
                format!("{:.1}分钟", p90 / 60000.0)
            } else if p90 >= 1000.0 {
                format!("{:.1}秒", p90 / 1000.0)
            } else {
                format!("{:.0}ms", p90)
            };

            Some(Diagnostic {
                rule_id: "REG001".to_string(),
                rule_name: "性能回归".to_string(),
                severity,
                node_path: "Query".to_string(),
                plan_node_id: None,
                message: format!(
                    "查询执行时间 {} 是历史 P90 ({}) 的 {:.1} 倍",
                    time_display, p90_display, ratio
                ),
                reason: format!(
                    "同类查询（{}）历史执行 {} 次，P50={:.0}ms P90={:.0}ms P99={:.0}ms，当前执行显著慢于历史表现。",
                    fingerprint.tables.join(", "),
                    baseline.time_stats.count(),
                    baseline.time_stats.p50(),
                    p90,
                    baseline.time_stats.p99()
                ),
                suggestions: vec![
                    "检查是否有数据分布变化导致执行计划改变".to_string(),
                    "检查是否有并发查询导致资源竞争".to_string(),
                    "检查相关表的统计信息是否过期".to_string(),
                    "对比历史执行计划是否有差异".to_string(),
                ],
                parameter_suggestions: vec![],
                threshold_metadata: None,
            })
        } else {
            None
        }
    }

    /// Get baseline for a fingerprint
    pub fn get_baseline(&self, fingerprint: &QueryFingerprint) -> Option<ExecutionBaseline> {
        self.cache.read().unwrap().get(&fingerprint.hash()).cloned()
    }

    /// Get all baselines (for debugging/monitoring)
    pub fn get_all_baselines(&self) -> Vec<ExecutionBaseline> {
        self.cache.read().unwrap().values().cloned().collect()
    }

    /// Get cache statistics
    pub fn stats(&self) -> HistoryStats {
        let cache = self.cache.read().unwrap();
        HistoryStats {
            cache_size: cache.len(),
            cache_capacity: self.config.memory_cache_size,
            total_samples: cache.values().map(|b| b.time_stats.count()).sum(),
        }
    }

    /// Clear all history (for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        self.cache.write().unwrap().clear();
        self.access_order.write().unwrap().clear();
    }
}

impl Default for QueryHistoryService {
    fn default() -> Self {
        Self::new()
    }
}

/// History service statistics
#[derive(Debug, Clone)]
pub struct HistoryStats {
    pub cache_size: usize,
    pub cache_capacity: usize,
    pub total_samples: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::profile_analyzer::models::{
        ExecutionInfo, PlannerInfo, Profile, ProfileSummary,
    };

    fn create_mock_profile(sql: &str, time_ms: f64) -> Profile {
        Profile {
            summary: ProfileSummary {
                sql_statement: sql.to_string(),
                total_time_ms: Some(time_ms),
                ..Default::default()
            },
            planner: PlannerInfo::default(),
            execution: ExecutionInfo::default(),
            fragments: vec![],
            execution_tree: None,
        }
    }

    #[test]
    fn test_fingerprint_normalization() {
        let fp1 = QueryFingerprint::from_profile(&create_mock_profile(
            "SELECT * FROM users WHERE id = 123",
            100.0,
        ));
        let fp2 = QueryFingerprint::from_profile(&create_mock_profile(
            "SELECT * FROM users WHERE id = 456",
            100.0,
        ));

        assert_eq!(fp1.hash(), fp2.hash(), "Same SQL template should have same hash");
        assert_eq!(fp1.tables, vec!["USERS"]);
    }

    #[test]
    fn test_regression_detection() {
        let config = HistoryConfig {
            min_samples_for_regression: 3,
            regression_ratio_threshold: 2.0,
            ..Default::default()
        };
        let service = QueryHistoryService::with_config(config);

        let sql = "SELECT * FROM orders WHERE user_id = 1";

        // Record baseline samples (100ms each)
        for _ in 0..5 {
            service.record_and_detect(&create_mock_profile(sql, 100.0));
        }

        // Should detect regression at 3x baseline
        let result = service.record_and_detect(&create_mock_profile(sql, 300.0));
        assert!(result.is_some(), "Should detect regression");
        assert_eq!(result.as_ref().unwrap().rule_id, "REG001");

        // Should not detect regression at 1.5x baseline
        service.clear();
        for _ in 0..5 {
            service.record_and_detect(&create_mock_profile(sql, 100.0));
        }
        let result = service.record_and_detect(&create_mock_profile(sql, 150.0));
        assert!(result.is_none(), "Should not detect regression at 1.5x");
    }

    #[test]
    fn test_lru_eviction() {
        let config = HistoryConfig { memory_cache_size: 3, ..Default::default() };
        let service = QueryHistoryService::with_config(config);

        // Fill cache
        service.record_and_detect(&create_mock_profile("SELECT * FROM a", 100.0));
        service.record_and_detect(&create_mock_profile("SELECT * FROM b", 100.0));
        service.record_and_detect(&create_mock_profile("SELECT * FROM c", 100.0));

        assert_eq!(service.stats().cache_size, 3);

        // Add one more, should evict oldest (a)
        service.record_and_detect(&create_mock_profile("SELECT * FROM d", 100.0));

        assert_eq!(service.stats().cache_size, 3);

        // Check that 'a' was evicted
        let fp_a = QueryFingerprint::from_profile(&create_mock_profile("SELECT * FROM a", 100.0));
        assert!(service.get_baseline(&fp_a).is_none(), "'a' should be evicted");
    }
}
