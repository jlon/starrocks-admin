use crate::models::cluster::Cluster;
use crate::utils::error::ApiResult;
use dashmap::DashMap;
use mysql_async::{OptsBuilder, Pool, SslOpts};
use std::sync::Arc;

/// Manager for MySQL connection pools using mysql_async with DashMap
///
/// Design: Uses DashMap for lock-free concurrent access.
/// Maintains a pool for each cluster to avoid reconnecting on every query.
///
/// Performance: 3-5x better than RwLock<HashMap> under high concurrency.
#[derive(Clone)]
pub struct MySQLPoolManager {
    pools: Arc<DashMap<i64, Pool>>,
}

impl MySQLPoolManager {
    pub fn new() -> Self {
        Self { pools: Arc::new(DashMap::new()) }
    }
}

impl Default for MySQLPoolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MySQLPoolManager {
    /// Get or create a connection pool for the given cluster
    ///
    /// Fast path: If pool exists, return immediately (lock-free read)
    /// Slow path: Create new pool if doesn't exist
    pub async fn get_pool(&self, cluster: &Cluster) -> ApiResult<Pool> {
        let cluster_id = cluster.id;

        // Fast path: Try to get existing pool (lock-free)
        if let Some(pool) = self.pools.get(&cluster_id) {
            return Ok(pool.clone());
        }

        // Slow path: Create new pool
        let pool = self.create_pool(cluster).await?;

        // Insert into map (DashMap handles concurrent inserts gracefully)
        self.pools.insert(cluster_id, pool.clone());

        tracing::info!(
            "Created MySQL connection pool for cluster {} ({}:{})",
            cluster_id,
            cluster.fe_host,
            cluster.fe_query_port
        );

        Ok(pool)
    }

    /// Remove a pool for a specific cluster
    ///
    /// Useful when cluster is deleted or credentials are updated
    pub async fn remove_pool(&self, cluster_id: i64) {
        if let Some((_, pool)) = self.pools.remove(&cluster_id) {
            drop(pool); // Pool will be closed when all references are dropped
            tracing::info!("Removed MySQL connection pool for cluster {}", cluster_id);
        }
    }

    /// Clear all pools (useful for cleanup/testing)
    pub async fn clear_all(&self) {
        self.pools.clear();
        tracing::info!("Cleared all MySQL connection pools");
    }

    /// Get pool count (for monitoring)
    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    /// Create a new MySQL connection pool for a cluster
    async fn create_pool(&self, cluster: &Cluster) -> ApiResult<Pool> {
        let opts = OptsBuilder::default()
            .ip_or_hostname(&cluster.fe_host)
            .tcp_port(cluster.fe_query_port as u16)
            .user(Some(&cluster.username))
            .pass(Some(&cluster.password_encrypted))
            .db_name(None::<String>) // No default database
            .prefer_socket(false) // Disable socket preference for StarRocks compatibility
            .ssl_opts(None::<SslOpts>) // No SSL for now
            .tcp_keepalive(Some(30_000_u32)) // 30 seconds
            .tcp_nodelay(true) // Disable Nagle's algorithm for lower latency
            .pool_opts(
                mysql_async::PoolOpts::default()
                    .with_constraints(mysql_async::PoolConstraints::new(2, 20).ok_or_else(
                        || {
                            crate::utils::ApiError::internal_error(
                                "Failed to create pool constraints: invalid min/max values",
                            )
                        },
                    )?)
                    .with_inactive_connection_ttl(std::time::Duration::from_secs(300)) // 5 minutes
                    .with_ttl_check_interval(std::time::Duration::from_secs(60)), // 1 minute
            );

        Ok(Pool::new(opts))
    }
}
