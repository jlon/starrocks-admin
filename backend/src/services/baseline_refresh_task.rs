//! Baseline Refresh Task
//!
//! Scheduled task for refreshing baseline data from audit logs.
//! Supports multi-cluster baselines with per-cluster isolation.
//! Uses the ScheduledExecutor framework for periodic execution.

use crate::services::baseline_service::BaselineService;
use crate::services::cluster_service::ClusterService;
use crate::services::mysql_pool_manager::MySQLPoolManager;
use crate::services::profile_analyzer::analyzer::{
    BaselineCacheManager, BaselineProvider, BaselineSource,
};
use crate::utils::scheduled_executor::ScheduledTask;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn};

// ============================================================================
// Multi-Cluster Baseline Refresh Task
// ============================================================================

/// Scheduled task for refreshing baseline data for ALL clusters
///
/// This task:
/// 1. Runs periodically (default: every hour)
/// 2. Fetches ALL enabled clusters
/// 3. For each cluster, fetches audit log data and calculates baselines
/// 4. Updates per-cluster cache
/// 5. Falls back to defaults on error for each cluster
pub struct BaselineRefreshTask {
    /// MySQL pool manager for database connections
    pool_manager: Arc<MySQLPoolManager>,
    /// Cluster service for getting clusters
    cluster_service: Arc<ClusterService>,
    /// Baseline service for calculations
    baseline_service: BaselineService,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
}

impl BaselineRefreshTask {
    /// Create a new baseline refresh task
    pub fn new(pool_manager: Arc<MySQLPoolManager>, cluster_service: Arc<ClusterService>) -> Self {
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

    /// Execute the refresh task for all clusters
    async fn execute(&self) -> Result<(), anyhow::Error> {
        info!("Starting multi-cluster baseline refresh...");

        // Get all enabled clusters
        let clusters = match self.cluster_service.list_clusters().await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to list clusters: {:?}", e);
                return Ok(());
            },
        };

        if clusters.is_empty() {
            info!("No clusters found, skipping baseline refresh");
            return Ok(());
        }

        let mut success_count = 0;
        let mut error_count = 0;

        for cluster in clusters {
            match self.refresh_cluster_baseline(&cluster).await {
                Ok(_) => {
                    success_count += 1;
                    info!(
                        "Baseline refresh completed for cluster: {} (id={})",
                        cluster.name, cluster.id
                    );
                },
                Err(e) => {
                    error_count += 1;
                    warn!("Baseline refresh failed for cluster {}: {}", cluster.name, e);
                    // Set default baselines for this cluster
                    BaselineProvider::update(
                        cluster.id,
                        BaselineCacheManager::default_baselines(),
                        BaselineSource::Default,
                    );
                },
            }
        }

        info!(
            "Multi-cluster baseline refresh completed: {} success, {} failed",
            success_count, error_count
        );

        Ok(())
    }

    /// Refresh baseline for a single cluster
    async fn refresh_cluster_baseline(
        &self,
        cluster: &crate::models::cluster::Cluster,
    ) -> Result<(), anyhow::Error> {
        // Get MySQL pool for the cluster
        let pool = self
            .pool_manager
            .get_pool(cluster)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get MySQL pool: {:?}", e))?;

        // Create MySQL client and refresh baselines
        let mysql = crate::services::mysql_client::MySQLClient::from_pool(pool);

        let result = self
            .baseline_service
            .refresh_from_audit_log_for_cluster(&mysql, cluster.id)
            .await
            .map_err(|e| anyhow::anyhow!("Baseline refresh failed: {}", e))?;

        info!(
            "Cluster {} baseline: source={:?}, samples={}",
            cluster.name, result.source, result.sample_count
        );

        Ok(())
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

    let executor = ScheduledExecutor::new("baseline-refresh", Duration::from_secs(interval_secs));

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
        let baseline = BaselineProvider::get_default(
            crate::services::profile_analyzer::analyzer::QueryComplexity::Medium,
        );
        assert!(baseline.stats.avg_ms > 0.0);
    }

    #[test]
    fn test_multi_cluster_baselines() {
        use crate::services::profile_analyzer::analyzer::QueryComplexity;

        // Initialize provider
        BaselineProvider::init();

        // Get baseline for cluster 1
        let baseline1 = BaselineProvider::get(1, QueryComplexity::Simple);
        assert!(baseline1.stats.avg_ms > 0.0);

        // Get baseline for cluster 2 (should also work, returns defaults)
        let baseline2 = BaselineProvider::get(2, QueryComplexity::Complex);
        assert!(baseline2.stats.avg_ms > 0.0);

        // Clusters should be independent
        assert_eq!(baseline1.complexity, QueryComplexity::Simple);
        assert_eq!(baseline2.complexity, QueryComplexity::Complex);
    }
}
