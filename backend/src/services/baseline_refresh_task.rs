//! Baseline Refresh Task
//!
//! Scheduled task for refreshing baseline data from audit logs.
//! Uses the ScheduledExecutor framework for periodic execution.

use crate::services::baseline_service::BaselineService;
use crate::services::mysql_pool_manager::MySQLPoolManager;
use crate::services::cluster_service::ClusterService;
use crate::services::profile_analyzer::analyzer::{BaselineProvider, BaselineSource, BaselineCacheManager};
use crate::utils::scheduled_executor::ScheduledTask;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn, error};

// ============================================================================
// Baseline Refresh Task
// ============================================================================

/// Scheduled task for refreshing baseline data
/// 
/// This task:
/// 1. Runs periodically (default: every hour)
/// 2. Fetches audit log data from active cluster
/// 3. Calculates baselines by query complexity
/// 4. Updates global cache
/// 5. Falls back to defaults on error
pub struct BaselineRefreshTask {
    /// MySQL pool manager for database connections
    pool_manager: Arc<MySQLPoolManager>,
    /// Cluster service for getting active cluster
    cluster_service: Arc<ClusterService>,
    /// Baseline service for calculations
    baseline_service: BaselineService,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
}

impl BaselineRefreshTask {
    /// Create a new baseline refresh task
    pub fn new(
        pool_manager: Arc<MySQLPoolManager>,
        cluster_service: Arc<ClusterService>,
    ) -> Self {
        // Initialize global baseline provider
        BaselineProvider::init();
        
        Self {
            pool_manager,
            cluster_service,
            baseline_service: BaselineService::new(),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Get shutdown handle
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }
    
    /// Execute the refresh task
    async fn execute(&self) -> Result<(), anyhow::Error> {
        info!("Starting baseline refresh...");
        
        // Get active cluster
        let cluster = match self.cluster_service.get_active_cluster().await {
            Ok(c) => c,
            Err(e) => {
                warn!("No active cluster found, using default baselines: {:?}", e);
                BaselineProvider::update(
                    BaselineCacheManager::default_baselines(),
                    BaselineSource::Default,
                );
                return Ok(());
            }
        };
        
        // Get MySQL pool for the cluster
        let pool = match self.pool_manager.get_pool(&cluster).await {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to get MySQL pool: {:?}", e);
                BaselineProvider::update(
                    BaselineCacheManager::default_baselines(),
                    BaselineSource::Default,
                );
                return Err(anyhow::anyhow!("Failed to get MySQL pool: {:?}", e));
            }
        };
        
        // Create MySQL client and refresh baselines
        let mysql = crate::services::mysql_client::MySQLClient::from_pool(pool);
        
        match self.baseline_service.refresh_from_audit_log(&mysql).await {
            Ok(result) => {
                info!(
                    "Baseline refresh completed: source={:?}, samples={}",
                    result.source, result.sample_count
                );
                Ok(())
            }
            Err(e) => {
                error!("Baseline refresh failed: {}", e);
                // Note: refresh_from_audit_log already sets default baselines on error
                Err(anyhow::anyhow!("Baseline refresh failed: {}", e))
            }
        }
    }
}

impl ScheduledTask for BaselineRefreshTask {
    fn run(&self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + '_>> {
        Box::pin(async move { self.execute().await })
    }
    
    fn should_terminate(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }
}

// ============================================================================
// Factory Function
// ============================================================================

/// Create and start the baseline refresh task
/// 
/// # Arguments
/// * `pool_manager` - MySQL pool manager
/// * `cluster_service` - Cluster service
/// * `interval_secs` - Refresh interval in seconds (default: 3600 = 1 hour)
/// 
/// # Returns
/// Shutdown handle for stopping the task
/// 
/// # Example
/// ```rust
/// let shutdown_handle = start_baseline_refresh_task(
///     pool_manager.clone(),
///     cluster_service.clone(),
///     3600, // 1 hour
/// );
/// 
/// // Later, to stop:
/// shutdown_handle.store(true, Ordering::Relaxed);
/// ```
pub fn start_baseline_refresh_task(
    pool_manager: Arc<MySQLPoolManager>,
    cluster_service: Arc<ClusterService>,
    interval_secs: u64,
) -> Arc<AtomicBool> {
    use crate::utils::scheduled_executor::ScheduledExecutor;
    use std::time::Duration;
    
    let task = BaselineRefreshTask::new(pool_manager, cluster_service);
    let shutdown_handle = task.shutdown_handle();
    
    let executor = ScheduledExecutor::new(
        "baseline-refresh",
        Duration::from_secs(interval_secs),
    );
    
    // Spawn the task
    tokio::spawn(async move {
        executor.start(task).await;
    });
    
    info!("Baseline refresh task started with interval: {}s", interval_secs);
    
    shutdown_handle
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_baselines_initialized() {
        // Provider should work even without initialization
        let baseline = BaselineProvider::get(
            crate::services::profile_analyzer::analyzer::QueryComplexity::Medium
        );
        assert!(baseline.stats.avg_ms > 0.0);
    }
}
