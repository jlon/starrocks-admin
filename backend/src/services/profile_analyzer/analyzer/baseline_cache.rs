//! Baseline Cache with Automatic Fallback
//!
//! Production-ready baseline management:
//! - In-memory cache with TTL (default: 1 hour)
//! - Per-cluster baseline isolation
//! - Background async refresh
//! - Graceful fallback to defaults when audit log unavailable
//! - Zero-allocation on cache hit

use super::baseline::{BaselineStats, PerformanceBaseline, QueryComplexity};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// ============================================================================
// Cached Baseline Data
// ============================================================================

/// Cached baseline data with metadata
#[derive(Debug, Clone)]
pub struct CachedBaseline {
    /// Cluster ID this baseline belongs to
    pub cluster_id: i64,
    /// Baseline data by complexity
    pub baselines: HashMap<QueryComplexity, PerformanceBaseline>,
    /// Cache creation time
    pub created_at: Instant,
    /// Cache TTL
    pub ttl: Duration,
    /// Data source
    pub source: BaselineSource,
}

/// Source of baseline data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineSource {
    /// From audit log (actual historical data)
    AuditLog,
    /// Default values (fallback)
    Default,
    /// From configuration file
    Config,
}

// ============================================================================
// Baseline Drift Detection
// ============================================================================

/// Result of baseline drift detection
#[derive(Debug, Clone)]
pub struct BaselineDriftResult {
    /// List of detected drifts
    pub drifts: Vec<DriftDetail>,
    /// When drift was detected
    pub detected_at: Instant,
}

/// Details of a single drift detection
#[derive(Debug, Clone)]
pub struct DriftDetail {
    /// Query complexity level
    pub complexity: QueryComplexity,
    /// Old P95 value (ms)
    pub old_p95_ms: f64,
    /// New P95 value (ms)
    pub new_p95_ms: f64,
    /// Ratio of new/old
    pub ratio: f64,
    /// Direction of drift
    pub direction: DriftDirection,
}

/// Direction of baseline drift
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftDirection {
    /// Performance degraded (new is slower)
    Slower,
    /// Performance improved (new is faster)
    Faster,
}

impl BaselineDriftResult {
    /// Check if any drift indicates performance degradation
    pub fn has_degradation(&self) -> bool {
        self.drifts
            .iter()
            .any(|d| d.direction == DriftDirection::Slower)
    }

    /// Get summary message
    pub fn summary(&self) -> String {
        let degraded: Vec<_> = self
            .drifts
            .iter()
            .filter(|d| d.direction == DriftDirection::Slower)
            .collect();

        if degraded.is_empty() {
            "基线检测到性能改善".to_string()
        } else {
            let details: Vec<String> = degraded
                .iter()
                .map(|d| {
                    format!(
                        "{:?}: P95 从 {:.0}ms 变为 {:.0}ms ({:.1}x)",
                        d.complexity, d.old_p95_ms, d.new_p95_ms, d.ratio
                    )
                })
                .collect();
            format!("⚠️ 基线漂移检测到性能下降: {}", details.join(", "))
        }
    }
}

impl CachedBaseline {
    /// Check if cache is still valid
    pub fn is_valid(&self) -> bool {
        self.created_at.elapsed() < self.ttl
    }

    /// Get baseline for specific complexity
    pub fn get(&self, complexity: QueryComplexity) -> Option<&PerformanceBaseline> {
        self.baselines.get(&complexity)
    }
}

// ============================================================================
// Multi-Cluster Baseline Cache Manager
// ============================================================================

/// Thread-safe baseline cache manager supporting multiple clusters
///
/// # Design Principles
/// 1. **Per-cluster isolation**: Each cluster has independent baselines
/// 2. **Zero-copy on read**: Uses RwLock for concurrent reads
/// 3. **Automatic fallback**: Returns defaults if cache miss
/// 4. **Background refresh**: Non-blocking cache updates
/// 5. **Graceful degradation**: Works without audit log
pub struct BaselineCacheManager {
    /// Cached baseline data per cluster (cluster_id -> CachedBaseline)
    cache: Arc<RwLock<HashMap<i64, CachedBaseline>>>,
    /// Default TTL (1 hour)
    default_ttl: Duration,
}

impl BaselineCacheManager {
    /// Create new cache manager
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Create with custom TTL
    pub fn with_ttl(ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(ttl_seconds),
        }
    }

    /// Get baseline for specific cluster and complexity (fast path)
    ///
    /// Returns:
    /// - Cached baseline if valid for this cluster
    /// - Default baseline if cache invalid or missing
    ///
    /// This method NEVER blocks on I/O - it always returns immediately
    pub fn get_baseline(
        &self,
        cluster_id: i64,
        complexity: QueryComplexity,
    ) -> PerformanceBaseline {
        // Fast path: read from cache
        if let Ok(cache_guard) = self.cache.read()
            && let Some(cached) = cache_guard.get(&cluster_id)
            && cached.is_valid()
            && let Some(baseline) = cached.get(complexity) {
                return baseline.clone();
        }

        // Fallback: return default baseline
        Self::default_baseline(complexity)
    }

    /// Get baseline without cluster (backward compatibility, uses cluster 0)
    pub fn get_baseline_default(&self, complexity: QueryComplexity) -> PerformanceBaseline {
        self.get_baseline(0, complexity)
    }

    /// Check if cache has valid data for cluster
    pub fn has_valid_cache(&self, cluster_id: i64) -> bool {
        if let Ok(cache_guard) = self.cache.read() {
            cache_guard
                .get(&cluster_id)
                .map(|c| c.is_valid())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Get cache source for cluster (for diagnostics)
    pub fn get_source(&self, cluster_id: i64) -> BaselineSource {
        if let Ok(cache_guard) = self.cache.read() {
            cache_guard
                .get(&cluster_id)
                .map(|c| c.source)
                .unwrap_or(BaselineSource::Default)
        } else {
            BaselineSource::Default
        }
    }

    /// Update cache with new baseline data for a specific cluster
    ///
    /// Called by background refresh task.
    /// Returns drift detection result if old baseline existed.
    pub fn update(
        &self,
        cluster_id: i64,
        baselines: HashMap<QueryComplexity, PerformanceBaseline>,
        source: BaselineSource,
    ) -> Option<BaselineDriftResult> {
        let drift_result = if let Ok(cache_guard) = self.cache.read() {
            cache_guard
                .get(&cluster_id)
                .and_then(|old| Self::detect_drift(&old.baselines, &baselines))
        } else {
            None
        };

        if let Ok(mut cache_guard) = self.cache.write() {
            cache_guard.insert(
                cluster_id,
                CachedBaseline {
                    cluster_id,
                    baselines,
                    created_at: Instant::now(),
                    ttl: self.default_ttl,
                    source,
                },
            );
        }

        drift_result
    }

    /// Detect baseline drift between old and new baselines
    /// Returns None if no significant drift detected
    fn detect_drift(
        old: &HashMap<QueryComplexity, PerformanceBaseline>,
        new: &HashMap<QueryComplexity, PerformanceBaseline>,
    ) -> Option<BaselineDriftResult> {
        let mut drifts = Vec::new();

        for (complexity, new_bl) in new {
            if let Some(old_bl) = old.get(complexity) {
                let ratio = new_bl.stats.p95_ms / old_bl.stats.p95_ms;
                // P95 变化超过 2 倍视为漂移
                if !(0.5..=2.0).contains(&ratio) {
                    drifts.push(DriftDetail {
                        complexity: *complexity,
                        old_p95_ms: old_bl.stats.p95_ms,
                        new_p95_ms: new_bl.stats.p95_ms,
                        ratio,
                        direction: if ratio > 1.0 {
                            DriftDirection::Slower
                        } else {
                            DriftDirection::Faster
                        },
                    });
                }
            }
        }

        if drifts.is_empty() {
            None
        } else {
            Some(BaselineDriftResult { drifts, detected_at: Instant::now() })
        }
    }

    /// Update cache for default cluster (backward compatibility)
    pub fn update_default(
        &self,
        baselines: HashMap<QueryComplexity, PerformanceBaseline>,
        source: BaselineSource,
    ) -> Option<BaselineDriftResult> {
        self.update(0, baselines, source)
    }

    /// Clear cache for a specific cluster
    pub fn clear(&self, cluster_id: i64) {
        if let Ok(mut cache_guard) = self.cache.write() {
            cache_guard.remove(&cluster_id);
        }
    }

    /// Clear all caches
    pub fn clear_all(&self) {
        if let Ok(mut cache_guard) = self.cache.write() {
            cache_guard.clear();
        }
    }

    /// Get list of cached cluster IDs
    pub fn cached_clusters(&self) -> Vec<i64> {
        if let Ok(cache_guard) = self.cache.read() {
            cache_guard.keys().cloned().collect()
        } else {
            vec![]
        }
    }

    /// Get default baseline for complexity
    ///
    /// These are conservative defaults based on industry best practices:
    /// - Simple queries: typically complete in < 5s
    /// - Medium queries: typically complete in < 15s
    /// - Complex queries: typically complete in < 60s
    /// - Very complex queries: may take several minutes
    pub fn default_baseline(complexity: QueryComplexity) -> PerformanceBaseline {
        let (avg, p50, p95, p99, max, std_dev) = match complexity {
            QueryComplexity::Simple => (2000.0, 1500.0, 4000.0, 6000.0, 10000.0, 1000.0),
            QueryComplexity::Medium => (5000.0, 4000.0, 10000.0, 15000.0, 30000.0, 3000.0),
            QueryComplexity::Complex => (15000.0, 12000.0, 30000.0, 45000.0, 90000.0, 8000.0),
            QueryComplexity::VeryComplex => {
                (45000.0, 35000.0, 90000.0, 120000.0, 300000.0, 20000.0)
            },
        };

        PerformanceBaseline {
            complexity,
            stats: BaselineStats {
                avg_ms: avg,
                p50_ms: p50,
                p95_ms: p95,
                p99_ms: p99,
                max_ms: max,
                std_dev_ms: std_dev,
            },
            sample_size: 0, // Indicates default data
            time_range_hours: 0,
        }
    }

    /// Get all default baselines
    pub fn default_baselines() -> HashMap<QueryComplexity, PerformanceBaseline> {
        let mut baselines = HashMap::new();
        for complexity in [
            QueryComplexity::Simple,
            QueryComplexity::Medium,
            QueryComplexity::Complex,
            QueryComplexity::VeryComplex,
        ] {
            baselines.insert(complexity, Self::default_baseline(complexity));
        }
        baselines
    }
}

impl Default for BaselineCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Baseline Provider
// ============================================================================

/// Global baseline provider with lazy initialization
///
/// Usage:
/// ```rust
/// // Get baseline for specific cluster
/// let baseline = BaselineProvider::get(cluster_id, QueryComplexity::Medium);
///
/// // Get baseline without cluster (uses default)
/// let baseline = BaselineProvider::get_default(QueryComplexity::Medium);
/// ```
pub struct BaselineProvider;

impl BaselineProvider {
    /// Get baseline for specific cluster and complexity
    ///
    /// This is the main entry point for getting baselines.
    /// It NEVER fails - always returns a valid baseline.
    pub fn get(cluster_id: i64, complexity: QueryComplexity) -> PerformanceBaseline {
        // Try global cache first
        if let Some(manager) = GLOBAL_CACHE.get() {
            manager.get_baseline(cluster_id, complexity)
        } else {
            // No global cache initialized, use defaults
            BaselineCacheManager::default_baseline(complexity)
        }
    }

    /// Get baseline without cluster (backward compatibility)
    pub fn get_default(complexity: QueryComplexity) -> PerformanceBaseline {
        Self::get(0, complexity)
    }

    /// Initialize global cache (call once at startup)
    pub fn init() {
        let _ = GLOBAL_CACHE.set(BaselineCacheManager::new());
    }

    /// Initialize with custom TTL
    pub fn init_with_ttl(ttl_seconds: u64) {
        let _ = GLOBAL_CACHE.set(BaselineCacheManager::with_ttl(ttl_seconds));
    }

    /// Update cache for specific cluster (called by background task)
    /// Returns drift detection result if old baseline existed
    pub fn update(
        cluster_id: i64,
        baselines: HashMap<QueryComplexity, PerformanceBaseline>,
        source: BaselineSource,
    ) -> Option<BaselineDriftResult> {
        GLOBAL_CACHE
            .get()
            .and_then(|manager| manager.update(cluster_id, baselines, source))
    }

    /// Update cache for default cluster (backward compatibility)
    pub fn update_default(
        baselines: HashMap<QueryComplexity, PerformanceBaseline>,
        source: BaselineSource,
    ) -> Option<BaselineDriftResult> {
        Self::update(0, baselines, source)
    }

    /// Check if audit log data is available for cluster
    pub fn has_audit_data(cluster_id: i64) -> bool {
        GLOBAL_CACHE
            .get()
            .map(|m| m.get_source(cluster_id) == BaselineSource::AuditLog)
            .unwrap_or(false)
    }

    /// Get list of cached cluster IDs
    pub fn cached_clusters() -> Vec<i64> {
        GLOBAL_CACHE
            .get()
            .map(|m| m.cached_clusters())
            .unwrap_or_default()
    }
}

/// Global cache instance
static GLOBAL_CACHE: std::sync::OnceLock<BaselineCacheManager> = std::sync::OnceLock::new();

// ============================================================================
// Baseline Refresh Task (for background updates)
// ============================================================================

/// Configuration for baseline refresh task
#[derive(Debug, Clone)]
pub struct BaselineRefreshConfig {
    /// Refresh interval (default: 1 hour)
    pub refresh_interval: Duration,
    /// Hours of audit log to analyze (default: 168 = 7 days)
    pub audit_log_hours: u32,
    /// Minimum sample size for valid baseline (default: 30)
    pub min_sample_size: usize,
    /// Whether to log refresh events
    pub enable_logging: bool,
}

impl Default for BaselineRefreshConfig {
    fn default() -> Self {
        Self {
            refresh_interval: Duration::from_secs(3600), // 1 hour
            audit_log_hours: 168,                        // 7 days
            min_sample_size: 30,
            enable_logging: true,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_baselines() {
        let baselines = BaselineCacheManager::default_baselines();

        // Should have all 4 complexity levels
        assert!(baselines.contains_key(&QueryComplexity::Simple));
        assert!(baselines.contains_key(&QueryComplexity::Medium));
        assert!(baselines.contains_key(&QueryComplexity::Complex));
        assert!(baselines.contains_key(&QueryComplexity::VeryComplex));

        // Simple should be faster than VeryComplex
        let simple = baselines.get(&QueryComplexity::Simple).unwrap();
        let very_complex = baselines.get(&QueryComplexity::VeryComplex).unwrap();
        assert!(simple.stats.avg_ms < very_complex.stats.avg_ms);
    }

    #[test]
    fn test_cache_manager_fallback() {
        let manager = BaselineCacheManager::new();
        let cluster_id = 1;

        // Without cache data, should return defaults
        let baseline = manager.get_baseline(cluster_id, QueryComplexity::Medium);
        assert_eq!(baseline.sample_size, 0); // Default indicator
        assert!(baseline.stats.avg_ms > 0.0);
    }

    #[test]
    fn test_cache_update_and_read() {
        let manager = BaselineCacheManager::new();
        let cluster_id = 1;

        // Create custom baseline
        let mut baselines = HashMap::new();
        baselines.insert(
            QueryComplexity::Medium,
            PerformanceBaseline {
                complexity: QueryComplexity::Medium,
                stats: BaselineStats {
                    avg_ms: 12345.0,
                    p50_ms: 10000.0,
                    p95_ms: 20000.0,
                    p99_ms: 25000.0,
                    max_ms: 30000.0,
                    std_dev_ms: 5000.0,
                },
                sample_size: 100,
                time_range_hours: 168,
            },
        );

        // Update cache for cluster
        manager.update(cluster_id, baselines, BaselineSource::AuditLog);

        // Read should return custom data
        let baseline = manager.get_baseline(cluster_id, QueryComplexity::Medium);
        assert_eq!(baseline.sample_size, 100);
        assert!((baseline.stats.avg_ms - 12345.0).abs() < 0.01);

        // Other cluster should return defaults
        let other_baseline = manager.get_baseline(999, QueryComplexity::Medium);
        assert_eq!(other_baseline.sample_size, 0);
    }

    #[test]
    fn test_cache_validity() {
        let manager = BaselineCacheManager::with_ttl(1); // 1 second TTL
        let cluster_id = 1;

        // Initially no cache
        assert!(!manager.has_valid_cache(cluster_id));

        // Update cache
        manager.update(cluster_id, HashMap::new(), BaselineSource::Default);
        assert!(manager.has_valid_cache(cluster_id));

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_secs(2));
        assert!(!manager.has_valid_cache(cluster_id));
    }

    #[test]
    fn test_cache_source() {
        let manager = BaselineCacheManager::new();
        let cluster_id = 1;

        // Default source (no cache)
        assert_eq!(manager.get_source(cluster_id), BaselineSource::Default);

        // Update with audit log data
        manager.update(cluster_id, HashMap::new(), BaselineSource::AuditLog);
        assert_eq!(manager.get_source(cluster_id), BaselineSource::AuditLog);
    }

    #[test]
    fn test_provider_fallback() {
        // Without initialization, should still work with defaults
        let baseline = BaselineProvider::get_default(QueryComplexity::Simple);
        assert!(baseline.stats.avg_ms > 0.0);
    }

    #[test]
    fn test_multi_cluster_isolation() {
        let manager = BaselineCacheManager::new();

        // Update cluster 1
        let mut baselines1 = HashMap::new();
        baselines1.insert(
            QueryComplexity::Simple,
            PerformanceBaseline {
                complexity: QueryComplexity::Simple,
                stats: BaselineStats {
                    avg_ms: 1000.0,
                    p50_ms: 800.0,
                    p95_ms: 2000.0,
                    p99_ms: 3000.0,
                    max_ms: 5000.0,
                    std_dev_ms: 500.0,
                },
                sample_size: 50,
                time_range_hours: 168,
            },
        );
        manager.update(1, baselines1, BaselineSource::AuditLog);

        // Update cluster 2
        let mut baselines2 = HashMap::new();
        baselines2.insert(
            QueryComplexity::Simple,
            PerformanceBaseline {
                complexity: QueryComplexity::Simple,
                stats: BaselineStats {
                    avg_ms: 5000.0, // Different avg
                    p50_ms: 4000.0,
                    p95_ms: 10000.0,
                    p99_ms: 15000.0,
                    max_ms: 20000.0,
                    std_dev_ms: 2000.0,
                },
                sample_size: 80,
                time_range_hours: 168,
            },
        );
        manager.update(2, baselines2, BaselineSource::AuditLog);

        // Verify isolation
        let b1 = manager.get_baseline(1, QueryComplexity::Simple);
        let b2 = manager.get_baseline(2, QueryComplexity::Simple);

        assert!((b1.stats.avg_ms - 1000.0).abs() < 0.01);
        assert!((b2.stats.avg_ms - 5000.0).abs() < 0.01);
        assert_eq!(b1.sample_size, 50);
        assert_eq!(b2.sample_size, 80);
    }
}
