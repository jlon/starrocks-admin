//! Baseline Cache with Automatic Fallback
//!
//! Production-ready baseline management:
//! - In-memory cache with TTL (default: 1 hour)
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
// Baseline Cache Manager
// ============================================================================

/// Thread-safe baseline cache manager
/// 
/// # Design Principles
/// 1. **Zero-copy on read**: Uses RwLock for concurrent reads
/// 2. **Automatic fallback**: Returns defaults if cache miss
/// 3. **Background refresh**: Non-blocking cache updates
/// 4. **Graceful degradation**: Works without audit log
pub struct BaselineCacheManager {
    /// Cached baseline data
    cache: Arc<RwLock<Option<CachedBaseline>>>,
    /// Default TTL (1 hour)
    default_ttl: Duration,
}

impl BaselineCacheManager {
    /// Create new cache manager
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
            default_ttl: Duration::from_secs(3600), // 1 hour
        }
    }
    
    /// Create with custom TTL
    pub fn with_ttl(ttl_seconds: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
            default_ttl: Duration::from_secs(ttl_seconds),
        }
    }
    
    /// Get baseline for specific complexity (fast path)
    /// 
    /// Returns:
    /// - Cached baseline if valid
    /// - Default baseline if cache invalid or missing
    /// 
    /// This method NEVER blocks on I/O - it always returns immediately
    pub fn get_baseline(&self, complexity: QueryComplexity) -> PerformanceBaseline {
        // Fast path: read from cache
        if let Ok(cache_guard) = self.cache.read() {
            if let Some(cached) = cache_guard.as_ref() {
                if cached.is_valid() {
                    if let Some(baseline) = cached.get(complexity) {
                        return baseline.clone();
                    }
                }
            }
        }
        
        // Fallback: return default baseline
        Self::default_baseline(complexity)
    }
    
    /// Check if cache has valid data
    pub fn has_valid_cache(&self) -> bool {
        if let Ok(cache_guard) = self.cache.read() {
            cache_guard.as_ref().map(|c| c.is_valid()).unwrap_or(false)
        } else {
            false
        }
    }
    
    /// Get cache source (for diagnostics)
    pub fn get_source(&self) -> BaselineSource {
        if let Ok(cache_guard) = self.cache.read() {
            cache_guard.as_ref().map(|c| c.source).unwrap_or(BaselineSource::Default)
        } else {
            BaselineSource::Default
        }
    }
    
    /// Update cache with new baseline data
    /// 
    /// Called by background refresh task
    pub fn update(&self, baselines: HashMap<QueryComplexity, PerformanceBaseline>, source: BaselineSource) {
        if let Ok(mut cache_guard) = self.cache.write() {
            *cache_guard = Some(CachedBaseline {
                baselines,
                created_at: Instant::now(),
                ttl: self.default_ttl,
                source,
            });
        }
    }
    
    /// Clear cache (for testing or manual refresh)
    pub fn clear(&self) {
        if let Ok(mut cache_guard) = self.cache.write() {
            *cache_guard = None;
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
            QueryComplexity::VeryComplex => (45000.0, 35000.0, 90000.0, 120000.0, 300000.0, 20000.0),
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
/// let baseline = BaselineProvider::get(QueryComplexity::Medium);
/// ```
pub struct BaselineProvider;

impl BaselineProvider {
    /// Get baseline for complexity (uses global cache or defaults)
    /// 
    /// This is the main entry point for getting baselines.
    /// It NEVER fails - always returns a valid baseline.
    pub fn get(complexity: QueryComplexity) -> PerformanceBaseline {
        // Try global cache first
        if let Some(manager) = GLOBAL_CACHE.get() {
            manager.get_baseline(complexity)
        } else {
            // No global cache initialized, use defaults
            BaselineCacheManager::default_baseline(complexity)
        }
    }
    
    /// Initialize global cache (call once at startup)
    pub fn init() {
        let _ = GLOBAL_CACHE.set(BaselineCacheManager::new());
    }
    
    /// Initialize with custom TTL
    pub fn init_with_ttl(ttl_seconds: u64) {
        let _ = GLOBAL_CACHE.set(BaselineCacheManager::with_ttl(ttl_seconds));
    }
    
    /// Update global cache (called by background task)
    pub fn update(baselines: HashMap<QueryComplexity, PerformanceBaseline>, source: BaselineSource) {
        if let Some(manager) = GLOBAL_CACHE.get() {
            manager.update(baselines, source);
        }
    }
    
    /// Check if audit log data is available
    pub fn has_audit_data() -> bool {
        GLOBAL_CACHE.get()
            .map(|m| m.get_source() == BaselineSource::AuditLog)
            .unwrap_or(false)
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
            audit_log_hours: 168, // 7 days
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
        
        // Without cache data, should return defaults
        let baseline = manager.get_baseline(QueryComplexity::Medium);
        assert_eq!(baseline.sample_size, 0); // Default indicator
        assert!(baseline.stats.avg_ms > 0.0);
    }
    
    #[test]
    fn test_cache_update_and_read() {
        let manager = BaselineCacheManager::new();
        
        // Create custom baseline
        let mut baselines = HashMap::new();
        baselines.insert(QueryComplexity::Medium, PerformanceBaseline {
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
        });
        
        // Update cache
        manager.update(baselines, BaselineSource::AuditLog);
        
        // Read should return custom data
        let baseline = manager.get_baseline(QueryComplexity::Medium);
        assert_eq!(baseline.sample_size, 100);
        assert!((baseline.stats.avg_ms - 12345.0).abs() < 0.01);
    }
    
    #[test]
    fn test_cache_validity() {
        let manager = BaselineCacheManager::with_ttl(1); // 1 second TTL
        
        // Initially no cache
        assert!(!manager.has_valid_cache());
        
        // Update cache
        manager.update(HashMap::new(), BaselineSource::Default);
        assert!(manager.has_valid_cache());
        
        // Wait for TTL to expire
        std::thread::sleep(Duration::from_secs(2));
        assert!(!manager.has_valid_cache());
    }
    
    #[test]
    fn test_cache_source() {
        let manager = BaselineCacheManager::new();
        
        // Default source
        assert_eq!(manager.get_source(), BaselineSource::Default);
        
        // Update with audit log data
        manager.update(HashMap::new(), BaselineSource::AuditLog);
        assert_eq!(manager.get_source(), BaselineSource::AuditLog);
    }
    
    #[test]
    fn test_provider_fallback() {
        // Without initialization, should still work with defaults
        let baseline = BaselineProvider::get(QueryComplexity::Simple);
        assert!(baseline.stats.avg_ms > 0.0);
    }
}
