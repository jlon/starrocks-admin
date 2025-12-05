// Metrics Collector Service
// Purpose: Periodically collect metrics from StarRocks clusters and store them in SQLite
// Design Ref: ARCHITECTURE_ANALYSIS_AND_INTEGRATION.md

use crate::models::Cluster;
use crate::services::mysql_pool_manager::MySQLPoolManager;
use crate::services::{ClusterService, StarRocksClient};
use crate::utils::{ApiResult, ScheduledTask};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use utoipa::ToSchema;

/// Aggregated metrics from database queries
#[derive(Debug, sqlx::FromRow)]
struct MetricsAggregation {
    avg_qps: Option<f64>,
    max_qps: Option<f64>,
    min_qps: Option<f64>,
    avg_latency_p99: Option<f64>,
    max_latency_p99: Option<f64>,
    total_queries: Option<i64>,
    total_errors: Option<i64>,
    avg_cpu_usage: Option<f64>,
    max_cpu_usage: Option<f64>,
    avg_memory_usage: Option<f64>,
    max_memory_usage: Option<f64>,
    avg_disk_usage_pct: Option<f64>,
    max_disk_usage_pct: Option<f64>,
    max_disk_used_bytes: Option<f64>,
}

/// Metrics snapshot stored in database
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct MetricsSnapshot {
    pub cluster_id: i64,
    pub collected_at: chrono::DateTime<Utc>,

    // Query performance
    pub qps: f64,
    pub rps: f64,
    pub query_latency_p50: f64,
    pub query_latency_p95: f64,
    pub query_latency_p99: f64,
    pub query_total: i64,
    pub query_success: i64,
    pub query_error: i64,
    pub query_timeout: i64,

    // Cluster health
    // Note: In shared-data mode, backend_* refers to Compute Nodes (CN)
    //       In shared-nothing mode, backend_* refers to Backend Nodes (BE)
    pub backend_total: i32,
    pub backend_alive: i32,
    pub frontend_total: i32,
    pub frontend_alive: i32,

    // Resource usage
    pub total_cpu_usage: f64,
    pub avg_cpu_usage: f64,
    pub total_memory_usage: f64,
    pub avg_memory_usage: f64,
    pub disk_total_bytes: i64,
    pub disk_used_bytes: i64,
    pub disk_usage_pct: f64,

    // Storage
    pub tablet_count: i64,
    pub max_compaction_score: f64,

    // Transactions
    pub txn_running: i32,
    pub txn_success_total: i64,
    pub txn_failed_total: i64,

    // Load jobs
    pub load_running: i32,
    pub load_finished_total: i64,

    // JVM metrics
    pub jvm_heap_total: i64,
    pub jvm_heap_used: i64,
    pub jvm_heap_usage_pct: f64,
    pub jvm_thread_count: i32,

    // Network metrics
    pub network_bytes_sent_total: i64,
    pub network_bytes_received_total: i64,
    pub network_send_rate: f64,
    pub network_receive_rate: f64,

    // IO metrics
    pub io_read_bytes_total: i64,
    pub io_write_bytes_total: i64,
    pub io_read_rate: f64,
    pub io_write_rate: f64,
}

#[derive(Clone)]
pub struct MetricsCollectorService {
    db: SqlitePool,
    cluster_service: Arc<ClusterService>,
    mysql_pool_manager: Arc<MySQLPoolManager>,
    retention_days: i64,
}

impl MetricsCollectorService {
    /// Create a new MetricsCollectorService
    pub fn new(
        db: SqlitePool,
        cluster_service: Arc<ClusterService>,
        mysql_pool_manager: Arc<MySQLPoolManager>,
        retention_days: i64,
    ) -> Self {
        Self { db, cluster_service, mysql_pool_manager, retention_days }
    }

    /// Execute one collection cycle
    /// This is called periodically by the ScheduledExecutor
    pub async fn collect_once(&self) -> Result<(), anyhow::Error> {
        // Collect metrics from all clusters
        self.collect_all_clusters().await?;

        // Check if we need to run daily aggregation
        self.check_and_run_daily_aggregation().await?;

        Ok(())
    }

    /// Check if daily aggregation is needed and run it
    async fn check_and_run_daily_aggregation(&self) -> Result<(), anyhow::Error> {
        // Check if we've already aggregated today
        let today = Utc::now().date_naive();
        let yesterday = today - chrono::Duration::days(1);

        // Check if yesterday's data has been aggregated
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) as count FROM daily_snapshots WHERE snapshot_date = ?")
                .bind(yesterday)
                .fetch_one(&self.db)
                .await?;

        if count.0 == 0 {
            tracing::info!("Running daily aggregation for date: {}", yesterday);
            self.run_daily_aggregation_all_clusters().await?;
        }

        Ok(())
    }

    /// Collect metrics from all clusters
    async fn collect_all_clusters(&self) -> Result<(), anyhow::Error> {
        let clusters = self.cluster_service.list_clusters().await?;

        tracing::debug!("Collecting metrics from {} clusters", clusters.len());

        for cluster in clusters {
            if let Err(e) = self.collect_cluster_metrics(&cluster).await {
                tracing::error!(
                    "Failed to collect metrics for cluster {} ({}): {}",
                    cluster.id,
                    cluster.name,
                    e
                );
                // Continue with other clusters
            }
        }

        // Cleanup old metrics every collection cycle
        if let Err(e) = self.cleanup_old_metrics().await {
            tracing::error!("Failed to cleanup old metrics: {}", e);
        }

        Ok(())
    }

    /// Collect metrics from a single cluster
    async fn collect_cluster_metrics(&self, cluster: &Cluster) -> ApiResult<()> {
        tracing::debug!("Collecting metrics for cluster: {} ({})", cluster.id, cluster.name);

        let client = StarRocksClient::new(cluster.clone(), self.mysql_pool_manager.clone());

        // Collect data from StarRocks
        let (metrics_text, backends, frontends, runtime_info) = tokio::try_join!(
            client.get_metrics(),
            client.get_backends(),
            client.get_frontends(),
            client.get_runtime_info(),
        )?;

        // Parse Prometheus metrics
        let metrics_map = client.parse_prometheus_metrics(&metrics_text)?;

        // Aggregate backend metrics (BE in shared-nothing, CN in shared-data)
        // Note: In shared-data mode, backends list contains Compute Nodes (CN), not Backend Nodes (BE)
        let backend_total = backends.len() as i32;
        let backend_alive = backends.iter().filter(|b| b.alive == "true").count() as i32;

        let frontend_total = frontends.len() as i32;
        let frontend_alive = frontends.iter().filter(|f| f.alive == "true").count() as i32;

        let tablet_count: i64 = backends
            .iter()
            .filter_map(|b| b.tablet_num.parse::<i64>().ok())
            .sum();

        let cpu_values: Vec<f64> = backends
            .iter()
            .filter_map(|b| {
                let trimmed = b.cpu_used_pct.trim().trim_end_matches('%').trim();
                match trimmed.parse::<f64>() {
                    Ok(v) => Some(v),
                    Err(_e) => {
                        tracing::warn!(
                            "Failed to parse CPU: '{}' from '{}'",
                            trimmed,
                            b.cpu_used_pct
                        );
                        None
                    },
                }
            })
            .collect();

        let total_cpu_usage: f64 = cpu_values.iter().sum();

        let avg_cpu_usage = if backend_total > 0 && !cpu_values.is_empty() {
            total_cpu_usage / cpu_values.len() as f64
        } else {
            0.0
        };

        tracing::debug!(
            "CPU parsing: parsed {}/{} compute nodes (BE/CN), total={}, avg={}",
            cpu_values.len(),
            backend_total,
            total_cpu_usage,
            avg_cpu_usage
        );

        let total_memory_usage: f64 = backends
            .iter()
            .filter_map(|b| {
                b.mem_used_pct
                    .trim()
                    .trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .ok()
            })
            .sum();

        let avg_memory_usage =
            if backend_total > 0 { total_memory_usage / backend_total as f64 } else { 0.0 };

        // Calculate disk total capacity (sum of all compute nodes: BE in shared-nothing, CN in shared-data)
        let disk_total_bytes: i64 = backends
            .iter()
            .filter_map(|b| parse_storage_size(&b.total_capacity))
            .sum();

        // For local cache usage: find the compute node with MAX disk usage percentage
        // and calculate its actual used bytes (represents cache pressure in shared-data or disk usage in shared-nothing)
        let (max_disk_usage_pct, _max_node_total, max_node_used) = backends
            .iter()
            .filter_map(|b| {
                let pct_str = b.max_disk_used_pct.trim().trim_end_matches('%').trim();
                let pct = pct_str.parse::<f64>().ok()?;
                let total = parse_storage_size(&b.total_capacity)?;
                let used = (total as f64 * pct / 100.0) as i64;
                Some((pct, total, used))
            })
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((0.0, 0, 0));

        // Use the max node's used bytes as disk_used_bytes (represents local cache usage)
        let disk_used_bytes = max_node_used;
        let disk_usage_pct = max_disk_usage_pct;

        tracing::debug!(
            "Disk usage (MAX node): {}% ({} bytes), total capacity: {} bytes (cluster mode: {})",
            disk_usage_pct,
            disk_used_bytes,
            disk_total_bytes,
            cluster.deployment_mode
        );

        // JVM metrics
        let jvm_heap_used = runtime_info.total_mem - runtime_info.free_mem;
        let jvm_heap_usage_pct = if runtime_info.total_mem > 0 {
            (jvm_heap_used as f64 / runtime_info.total_mem as f64) * 100.0
        } else {
            0.0
        };

        // Network metrics (BE)
        let network_bytes_sent_total = metrics_map
            .get("starrocks_be_network_send_bytes")
            .copied()
            .unwrap_or(0.0) as i64;
        let network_bytes_received_total = metrics_map
            .get("starrocks_be_network_receive_bytes")
            .copied()
            .unwrap_or(0.0) as i64;
        let network_send_rate = metrics_map
            .get("starrocks_be_network_send_rate")
            .copied()
            .unwrap_or(0.0);
        let network_receive_rate = metrics_map
            .get("starrocks_be_network_receive_rate")
            .copied()
            .unwrap_or(0.0);

        // IO metrics (BE)
        let io_read_bytes_total = metrics_map
            .get("starrocks_be_disk_read_bytes")
            .copied()
            .unwrap_or(0.0) as i64;
        let io_write_bytes_total = metrics_map
            .get("starrocks_be_disk_write_bytes")
            .copied()
            .unwrap_or(0.0) as i64;
        let io_read_rate = metrics_map
            .get("starrocks_be_disk_read_rate")
            .copied()
            .unwrap_or(0.0);
        let io_write_rate = metrics_map
            .get("starrocks_be_disk_write_rate")
            .copied()
            .unwrap_or(0.0);

        // Get real latency percentiles from audit logs using StarRocks percentile functions
        let (real_p50, real_p95, real_p99) = self
            .get_real_latency_percentiles(cluster)
            .await
            .unwrap_or((0.0, 0.0, 0.0));

        // Create snapshot
        let snapshot = MetricsSnapshot {
            cluster_id: cluster.id,
            collected_at: Utc::now(),

            // Query metrics: Use real percentiles from audit logs, fallback to Prometheus
            qps: metrics_map.get("starrocks_fe_qps").copied().unwrap_or(0.0),
            rps: metrics_map.get("starrocks_fe_rps").copied().unwrap_or(0.0),
            query_latency_p50: if real_p50 > 0.0 {
                real_p50
            } else {
                metrics_map
                    .get("starrocks_fe_query_latency_p50")
                    .copied()
                    .unwrap_or(0.0)
            },
            query_latency_p95: if real_p95 > 0.0 {
                real_p95
            } else {
                metrics_map
                    .get("starrocks_fe_query_latency_p95")
                    .copied()
                    .unwrap_or(0.0)
            },
            query_latency_p99: if real_p99 > 0.0 {
                real_p99
            } else {
                metrics_map
                    .get("starrocks_fe_query_latency_p99")
                    .copied()
                    .unwrap_or(0.0)
            },
            query_total: metrics_map
                .get("starrocks_fe_query_total")
                .copied()
                .unwrap_or(0.0) as i64,
            query_success: metrics_map
                .get("starrocks_fe_query_success")
                .copied()
                .unwrap_or(0.0) as i64,
            query_error: metrics_map
                .get("starrocks_fe_query_err")
                .copied()
                .unwrap_or(0.0) as i64,
            query_timeout: metrics_map
                .get("starrocks_fe_query_timeout")
                .copied()
                .unwrap_or(0.0) as i64,

            // Cluster health
            backend_total,
            backend_alive,
            frontend_total,
            frontend_alive,

            // Resource usage
            total_cpu_usage,
            avg_cpu_usage,
            total_memory_usage,
            avg_memory_usage,
            disk_total_bytes,
            disk_used_bytes,
            disk_usage_pct,

            // Storage
            tablet_count,
            max_compaction_score: metrics_map
                .get("starrocks_fe_max_tablet_compaction_score")
                .copied()
                .unwrap_or(0.0),

            // Transactions
            txn_running: 0, // TODO: Need to get from appropriate metric
            txn_success_total: metrics_map
                .get("starrocks_fe_txn_success")
                .copied()
                .unwrap_or(0.0) as i64,
            txn_failed_total: metrics_map
                .get("starrocks_fe_txn_failed")
                .copied()
                .unwrap_or(0.0) as i64,

            // Load jobs
            load_running: 0, // TODO: Need to get from appropriate metric
            load_finished_total: metrics_map
                .get("starrocks_fe_load_finished")
                .copied()
                .unwrap_or(0.0) as i64,

            // JVM metrics
            jvm_heap_total: runtime_info.total_mem,
            jvm_heap_used,
            jvm_heap_usage_pct,
            jvm_thread_count: runtime_info.thread_cnt,

            // Network metrics
            network_bytes_sent_total,
            network_bytes_received_total,
            network_send_rate,
            network_receive_rate,

            // IO metrics
            io_read_bytes_total,
            io_write_bytes_total,
            io_read_rate,
            io_write_rate,
        };

        // Save to database
        self.save_snapshot(&snapshot).await?;

        tracing::debug!(
            "Metrics collected for cluster {} ({}): QPS={:.2}, CPU={:.1}%, Disk={:.1}%",
            cluster.id,
            cluster.name,
            snapshot.qps,
            snapshot.avg_cpu_usage,
            snapshot.disk_usage_pct
        );

        Ok(())
    }

    /// Save metrics snapshot to database
    async fn save_snapshot(&self, snapshot: &MetricsSnapshot) -> ApiResult<()> {
        sqlx::query(
            r#"
            INSERT INTO metrics_snapshots (
                cluster_id, collected_at,
                qps, rps, query_latency_p50, query_latency_p95, query_latency_p99,
                query_total, query_success, query_error, query_timeout,
                backend_total, backend_alive, frontend_total, frontend_alive,
                total_cpu_usage, avg_cpu_usage, total_memory_usage, avg_memory_usage,
                disk_total_bytes, disk_used_bytes, disk_usage_pct,
                tablet_count, max_compaction_score,
                txn_running, txn_success_total, txn_failed_total,
                load_running, load_finished_total,
                jvm_heap_total, jvm_heap_used, jvm_heap_usage_pct, jvm_thread_count,
                network_bytes_sent_total, network_bytes_received_total, network_send_rate, network_receive_rate,
                io_read_bytes_total, io_write_bytes_total, io_read_rate, io_write_rate,
                raw_metrics
            ) VALUES (
                ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?,
                ?, ?, ?,
                ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?
            )
            "#
        )
        .bind(snapshot.cluster_id)
        .bind(snapshot.collected_at)
        .bind(snapshot.qps)
        .bind(snapshot.rps)
        .bind(snapshot.query_latency_p50)
        .bind(snapshot.query_latency_p95)
        .bind(snapshot.query_latency_p99)
        .bind(snapshot.query_total)
        .bind(snapshot.query_success)
        .bind(snapshot.query_error)
        .bind(snapshot.query_timeout)
        .bind(snapshot.backend_total)
        .bind(snapshot.backend_alive)
        .bind(snapshot.frontend_total)
        .bind(snapshot.frontend_alive)
        .bind(snapshot.total_cpu_usage)
        .bind(snapshot.avg_cpu_usage)
        .bind(snapshot.total_memory_usage)
        .bind(snapshot.avg_memory_usage)
        .bind(snapshot.disk_total_bytes)
        .bind(snapshot.disk_used_bytes)
        .bind(snapshot.disk_usage_pct)
        .bind(snapshot.tablet_count)
        .bind(snapshot.max_compaction_score)
        .bind(snapshot.txn_running)
        .bind(snapshot.txn_success_total)
        .bind(snapshot.txn_failed_total)
        .bind(snapshot.load_running)
        .bind(snapshot.load_finished_total)
        .bind(snapshot.jvm_heap_total)
        .bind(snapshot.jvm_heap_used)
        .bind(snapshot.jvm_heap_usage_pct)
        .bind(snapshot.jvm_thread_count)
        .bind(snapshot.network_bytes_sent_total)
        .bind(snapshot.network_bytes_received_total)
        .bind(snapshot.network_send_rate)
        .bind(snapshot.network_receive_rate)
        .bind(snapshot.io_read_bytes_total)
        .bind(snapshot.io_write_bytes_total)
        .bind(snapshot.io_read_rate)
        .bind(snapshot.io_write_rate)
        .bind(None::<String>) // raw_metrics: reserved for future use
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Cleanup old metrics data based on retention policy
    async fn cleanup_old_metrics(&self) -> Result<(), sqlx::Error> {
        let cutoff_date = Utc::now() - chrono::Duration::days(self.retention_days);

        let result = sqlx::query("DELETE FROM metrics_snapshots WHERE collected_at < ?")
            .bind(cutoff_date)
            .execute(&self.db)
            .await?;

        if result.rows_affected() > 0 {
            tracing::info!(
                "Cleaned up {} old metric snapshots (older than {} days)",
                result.rows_affected(),
                self.retention_days
            );
        }

        Ok(())
    }

    /// Get the latest snapshot for a cluster
    pub async fn get_latest_snapshot(&self, cluster_id: i64) -> ApiResult<Option<MetricsSnapshot>> {
        #[derive(sqlx::FromRow)]
        struct SnapshotRow {
            cluster_id: i64,
            collected_at: chrono::NaiveDateTime,
            qps: f64,
            rps: f64,
            query_latency_p50: f64,
            query_latency_p95: f64,
            query_latency_p99: f64,
            query_total: i64,
            query_success: i64,
            query_error: i64,
            query_timeout: i64,
            backend_total: i64,
            backend_alive: i64,
            frontend_total: i64,
            frontend_alive: i64,
            total_cpu_usage: f64,
            avg_cpu_usage: f64,
            total_memory_usage: f64,
            avg_memory_usage: f64,
            disk_total_bytes: i64,
            disk_used_bytes: i64,
            disk_usage_pct: f64,
            tablet_count: i64,
            max_compaction_score: f64,
            txn_running: i64,
            txn_success_total: i64,
            txn_failed_total: i64,
            load_running: i64,
            load_finished_total: i64,
            jvm_heap_total: i64,
            jvm_heap_used: i64,
            jvm_heap_usage_pct: f64,
            jvm_thread_count: i64,
            network_bytes_sent_total: i64,
            network_bytes_received_total: i64,
            network_send_rate: f64,
            network_receive_rate: f64,
            io_read_bytes_total: i64,
            io_write_bytes_total: i64,
            io_read_rate: f64,
            io_write_rate: f64,
        }

        let row: Option<SnapshotRow> = sqlx::query_as(
            r#"
            SELECT * FROM metrics_snapshots
            WHERE cluster_id = ?
            ORDER BY collected_at DESC
            LIMIT 1
            "#,
        )
        .bind(cluster_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some(r) = row {
            Ok(Some(MetricsSnapshot {
                cluster_id: r.cluster_id,
                collected_at: r.collected_at.and_utc(),
                qps: r.qps,
                rps: r.rps,
                query_latency_p50: r.query_latency_p50,
                query_latency_p95: r.query_latency_p95,
                query_latency_p99: r.query_latency_p99,
                query_total: r.query_total,
                query_success: r.query_success,
                query_error: r.query_error,
                query_timeout: r.query_timeout,
                backend_total: r.backend_total as i32,
                backend_alive: r.backend_alive as i32,
                frontend_total: r.frontend_total as i32,
                frontend_alive: r.frontend_alive as i32,
                total_cpu_usage: r.total_cpu_usage,
                avg_cpu_usage: r.avg_cpu_usage,
                total_memory_usage: r.total_memory_usage,
                avg_memory_usage: r.avg_memory_usage,
                disk_total_bytes: r.disk_total_bytes,
                disk_used_bytes: r.disk_used_bytes,
                disk_usage_pct: r.disk_usage_pct,
                tablet_count: r.tablet_count,
                max_compaction_score: r.max_compaction_score,
                txn_running: r.txn_running as i32,
                txn_success_total: r.txn_success_total,
                txn_failed_total: r.txn_failed_total,
                load_running: r.load_running as i32,
                load_finished_total: r.load_finished_total,
                jvm_heap_total: r.jvm_heap_total,
                jvm_heap_used: r.jvm_heap_used,
                jvm_heap_usage_pct: r.jvm_heap_usage_pct,
                jvm_thread_count: r.jvm_thread_count as i32,
                network_bytes_sent_total: r.network_bytes_sent_total,
                network_bytes_received_total: r.network_bytes_received_total,
                network_send_rate: r.network_send_rate,
                network_receive_rate: r.network_receive_rate,
                io_read_bytes_total: r.io_read_bytes_total,
                io_write_bytes_total: r.io_write_bytes_total,
                io_read_rate: r.io_read_rate,
                io_write_rate: r.io_write_rate,
            }))
        } else {
            Ok(None)
        }
    }
}

// Helper function to parse storage size strings like "1.5 TB", "500 GB", etc.
fn parse_storage_size(size_str: &str) -> Option<i64> {
    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let value: f64 = parts[0].parse().ok()?;
    let unit = parts[1].to_uppercase();

    let bytes = match unit.as_str() {
        "B" | "BYTES" => value,
        "KB" => value * 1024.0,
        "MB" => value * 1024.0 * 1024.0,
        "GB" => value * 1024.0 * 1024.0 * 1024.0,
        "TB" => value * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "PB" => value * 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };

    Some(bytes as i64)
}

impl MetricsCollectorService {
    /// Run daily aggregation for all clusters
    async fn run_daily_aggregation_all_clusters(&self) -> Result<(), anyhow::Error> {
        let clusters = self.cluster_service.list_clusters().await?;

        tracing::info!("Starting daily aggregation for {} clusters", clusters.len());

        for cluster in clusters {
            if let Err(e) = self.run_daily_aggregation_for_cluster(cluster.id).await {
                tracing::error!(
                    "Failed to run daily aggregation for cluster {} ({}): {}",
                    cluster.id,
                    cluster.name,
                    e
                );
            }
        }

        // Cleanup old daily snapshots (keep 90 days)
        self.cleanup_old_daily_snapshots().await?;

        Ok(())
    }

    /// Run daily aggregation for a single cluster
    /// Aggregates yesterday's metrics_snapshots into daily_snapshots
    async fn run_daily_aggregation_for_cluster(&self, cluster_id: i64) -> ApiResult<()> {
        let yesterday = Utc::now().date_naive() - chrono::Duration::days(1);
        let yesterday_start = yesterday.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let yesterday_end = yesterday.and_hms_opt(23, 59, 59).unwrap().and_utc();

        tracing::debug!(
            "Aggregating metrics for cluster {} from {} to {}",
            cluster_id,
            yesterday_start,
            yesterday_end
        );

        // Query all snapshots from yesterday
        let snapshots = sqlx::query_as::<_, MetricsAggregation>(
            r#"
            SELECT
                AVG(qps) as avg_qps,
                MAX(qps) as max_qps,
                MIN(qps) as min_qps,
                AVG(query_latency_p99) as avg_latency_p99,
                MAX(query_latency_p99) as max_latency_p99,
                SUM(query_total) as total_queries,
                SUM(query_error) as total_errors,
                AVG(avg_cpu_usage) as avg_cpu_usage,
                MAX(avg_cpu_usage) as max_cpu_usage,
                AVG(avg_memory_usage) as avg_memory_usage,
                MAX(avg_memory_usage) as max_memory_usage,
                AVG(disk_usage_pct) as avg_disk_usage_pct,
                MAX(disk_usage_pct) as max_disk_usage_pct,
                CAST(MAX(disk_used_bytes) AS REAL) as max_disk_used_bytes
            FROM metrics_snapshots
            WHERE cluster_id = ?
                AND collected_at >= ?
                AND collected_at <= ?
            "#,
        )
        .bind(cluster_id)
        .bind(yesterday_start)
        .bind(yesterday_end)
        .fetch_one(&self.db)
        .await?;

        // Calculate derived metrics
        let total_queries = snapshots.total_queries.unwrap_or(0);
        let total_errors = snapshots.total_errors.unwrap_or(0);
        let error_rate = if total_queries > 0 {
            (total_errors as f64 / total_queries as f64) * 100.0
        } else {
            0.0
        };

        // Prepare values (to avoid temporary value lifetime issues)
        let avg_qps = snapshots.avg_qps.unwrap_or(0.0);
        let max_qps = snapshots.max_qps.unwrap_or(0.0);
        let min_qps = snapshots.min_qps.unwrap_or(0.0);
        let avg_latency_p99 = snapshots.avg_latency_p99.unwrap_or(0.0);
        let max_latency_p99 = snapshots.max_latency_p99.unwrap_or(0.0);
        let avg_cpu_usage = snapshots.avg_cpu_usage.unwrap_or(0.0);
        let max_cpu_usage = snapshots.max_cpu_usage.unwrap_or(0.0);
        let avg_memory_usage = snapshots.avg_memory_usage.unwrap_or(0.0);
        let max_memory_usage = snapshots.max_memory_usage.unwrap_or(0.0);
        let avg_disk_usage_pct = snapshots.avg_disk_usage_pct.unwrap_or(0.0);
        let max_disk_usage_pct = snapshots.max_disk_usage_pct.unwrap_or(0.0);
        let data_size_end = snapshots.max_disk_used_bytes.unwrap_or(0.0) as i64;
        let data_growth_bytes = 0i64; // Would need previous day's value

        // Insert or update daily snapshot
        sqlx::query(
            r#"
            INSERT INTO daily_snapshots (
                cluster_id, snapshot_date,
                avg_qps, max_qps, min_qps,
                avg_latency_p99, max_latency_p99,
                total_queries, total_errors, error_rate,
                avg_cpu_usage, max_cpu_usage,
                avg_memory_usage, max_memory_usage,
                avg_disk_usage_pct, max_disk_usage_pct,
                data_size_end, data_growth_bytes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(cluster_id, snapshot_date) DO UPDATE SET
                avg_qps = excluded.avg_qps,
                max_qps = excluded.max_qps,
                min_qps = excluded.min_qps,
                avg_latency_p99 = excluded.avg_latency_p99,
                max_latency_p99 = excluded.max_latency_p99,
                total_queries = excluded.total_queries,
                total_errors = excluded.total_errors,
                error_rate = excluded.error_rate,
                avg_cpu_usage = excluded.avg_cpu_usage,
                max_cpu_usage = excluded.max_cpu_usage,
                avg_memory_usage = excluded.avg_memory_usage,
                max_memory_usage = excluded.max_memory_usage,
                avg_disk_usage_pct = excluded.avg_disk_usage_pct,
                max_disk_usage_pct = excluded.max_disk_usage_pct,
                data_size_end = excluded.data_size_end,
                data_growth_bytes = excluded.data_growth_bytes
            "#,
        )
        .bind(cluster_id)
        .bind(yesterday)
        .bind(avg_qps)
        .bind(max_qps)
        .bind(min_qps)
        .bind(avg_latency_p99)
        .bind(max_latency_p99)
        .bind(total_queries)
        .bind(total_errors)
        .bind(error_rate)
        .bind(avg_cpu_usage)
        .bind(max_cpu_usage)
        .bind(avg_memory_usage)
        .bind(max_memory_usage)
        .bind(avg_disk_usage_pct)
        .bind(max_disk_usage_pct)
        .bind(data_size_end)
        .bind(data_growth_bytes)
        .execute(&self.db)
        .await?;

        tracing::info!(
            "Daily aggregation completed for cluster {} (date: {})",
            cluster_id,
            yesterday
        );

        Ok(())
    }

    /// Detect the available query time column name in audit log table
    /// Different StarRocks versions may use different column names
    async fn detect_query_time_column(
        &self,
        mysql_client: &crate::services::mysql_client::MySQLClient,
    ) -> Option<String> {
        // First, try to get column list using SHOW COLUMNS
        let show_columns_query = "SHOW COLUMNS FROM starrocks_audit_db__.starrocks_audit_tbl__";

        match mysql_client.query_raw(show_columns_query).await {
            Ok((columns, rows)) => {
                // Find the column index for "Field" column
                let field_idx = columns
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case("Field"))?;

                // Possible column names to look for (in order of likelihood)
                let possible_columns =
                    vec!["queryTime", "query_time", "duration", "QueryTime", "queryDuration"];

                // Check each row for matching column name
                for row in rows {
                    if let Some(col_name) = row.get(field_idx) {
                        let col_name_lower = col_name.to_lowercase();
                        for possible_col in &possible_columns {
                            if col_name_lower == possible_col.to_lowercase() {
                                tracing::debug!("Detected query time column: {}", col_name);
                                return Some(col_name.clone());
                            }
                        }
                    }
                }
            },
            Err(e) => {
                tracing::debug!("Failed to get column list, trying fallback method: {}", e);
                // Fallback: try direct queries with common column names
                let possible_columns =
                    vec!["queryTime", "query_time", "duration", "QueryTime", "queryDuration"];

                for col_name in possible_columns {
                    // Use a safe query that won't fail if column doesn't exist
                    let test_query = format!(
                        "SELECT COUNT(*) as cnt FROM starrocks_audit_db__.starrocks_audit_tbl__ WHERE {} > 0 LIMIT 1",
                        col_name
                    );

                    if mysql_client.query(&test_query).await.is_ok() {
                        tracing::debug!("Detected query time column (fallback): {}", col_name);
                        return Some(col_name.to_string());
                    }
                }
            },
        }

        tracing::warn!(
            "Could not detect query time column in audit log table. Available columns may differ by StarRocks version."
        );
        None
    }

    /// Get real latency percentiles from audit logs using StarRocks percentile functions
    /// Reference: https://docs.starrocks.io/zh/docs/category/percentile/
    async fn get_real_latency_percentiles(&self, cluster: &Cluster) -> ApiResult<(f64, f64, f64)> {
        use crate::services::mysql_client::MySQLClient;

        // Get MySQL connection pool and create client
        let pool = self.mysql_pool_manager.get_pool(cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        // Detect the available query time column name
        let query_time_col = match self.detect_query_time_column(&mysql_client).await {
            Some(col) => col,
            None => {
                tracing::debug!(
                    "Query time column not found, skipping audit log percentile calculation"
                );
                return Ok((0.0, 0.0, 0.0)); // Return zeros to fallback to Prometheus
            },
        };

        // Use StarRocks percentile_approx function to calculate P50, P95, P99 from audit logs
        // Query recent 3 days of data for comprehensive percentiles
        let query = format!(
            r#"
            SELECT 
                COALESCE(percentile_approx({}, 0.50), 0) as p50,
                COALESCE(percentile_approx({}, 0.95), 0) as p95,
                COALESCE(percentile_approx({}, 0.99), 0) as p99
            FROM starrocks_audit_db__.starrocks_audit_tbl__
            WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 3 DAY)
                AND {} > 0
                AND state = 'EOF'
                AND isQuery = 1
        "#,
            query_time_col, query_time_col, query_time_col, query_time_col
        );

        match mysql_client.query(&query).await {
            Ok(results) => {
                if let Some(row) = results.first() {
                    // Parse percentile values - percentile_approx returns DOUBLE, so try f64 first
                    let p50 = row
                        .get("p50")
                        .and_then(|v| v.as_f64())
                        .or_else(|| row.get("p50").and_then(|v| v.as_i64()).map(|i| i as f64))
                        .or_else(|| {
                            row.get("p50")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<f64>().ok())
                        })
                        .unwrap_or(0.0);

                    let p95 = row
                        .get("p95")
                        .and_then(|v| v.as_f64())
                        .or_else(|| row.get("p95").and_then(|v| v.as_i64()).map(|i| i as f64))
                        .or_else(|| {
                            row.get("p95")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<f64>().ok())
                        })
                        .unwrap_or(0.0);

                    let p99 = row
                        .get("p99")
                        .and_then(|v| v.as_f64())
                        .or_else(|| row.get("p99").and_then(|v| v.as_i64()).map(|i| i as f64))
                        .or_else(|| {
                            row.get("p99")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<f64>().ok())
                        })
                        .unwrap_or(0.0);

                    tracing::debug!(
                        "Real latency percentiles from audit logs: P50={:.2}ms, P95={:.2}ms, P99={:.2}ms",
                        p50,
                        p95,
                        p99
                    );

                    Ok((p50, p95, p99))
                } else {
                    tracing::debug!("No audit log data available for percentile calculation");
                    Ok((0.0, 0.0, 0.0))
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to query audit logs for percentiles: {}. Falling back to Prometheus metrics.",
                    e
                );
                Ok((0.0, 0.0, 0.0)) // Return zeros to fallback to Prometheus
            },
        }
    }

    /// Cleanup old daily snapshots (keep 90 days)
    async fn cleanup_old_daily_snapshots(&self) -> Result<(), sqlx::Error> {
        let cutoff_date = Utc::now().date_naive() - chrono::Duration::days(90);

        let result = sqlx::query("DELETE FROM daily_snapshots WHERE snapshot_date < ?")
            .bind(cutoff_date)
            .execute(&self.db)
            .await?;

        if result.rows_affected() > 0 {
            tracing::info!(
                "Cleaned up {} old daily snapshots (older than 90 days)",
                result.rows_affected()
            );
        }

        Ok(())
    }
}

// Implement ScheduledTask for MetricsCollectorService
impl ScheduledTask for MetricsCollectorService {
    fn run(&self) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + '_>> {
        Box::pin(async move { self.collect_once().await })
    }
}
