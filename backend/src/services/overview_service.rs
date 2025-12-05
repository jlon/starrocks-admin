// Overview Service
// Purpose: Provide aggregated cluster overview data (real-time + historical)
// Design Ref: ARCHITECTURE_ANALYSIS_AND_INTEGRATION.md

use crate::services::{
    ClusterService, DataStatistics, DataStatisticsService, MetricsSnapshot, MySQLClient,
};
use crate::utils::{ApiError, ApiResult};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use utoipa::ToSchema;

/// Time range for querying historical data
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeRange {
    #[serde(rename = "1h")]
    Hours1,
    #[serde(rename = "6h")]
    Hours6,
    #[serde(rename = "24h")]
    Hours24,
    #[serde(rename = "3d")]
    Days3,
}

impl TimeRange {
    pub fn to_duration(&self) -> chrono::Duration {
        match self {
            TimeRange::Hours1 => chrono::Duration::hours(1),
            TimeRange::Hours6 => chrono::Duration::hours(6),
            TimeRange::Hours24 => chrono::Duration::hours(24),
            TimeRange::Days3 => chrono::Duration::days(3),
        }
    }

    pub fn start_time(&self) -> DateTime<Utc> {
        Utc::now() - self.to_duration()
    }

    pub fn end_time(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Cluster overview data
#[derive(Debug, Serialize, ToSchema)]
pub struct ClusterOverview {
    pub cluster_id: i64,
    pub cluster_name: String,
    pub timestamp: DateTime<Utc>,

    // Real-time snapshot
    pub latest_snapshot: Option<MetricsSnapshot>,

    // Historical trends (time series data)
    pub performance_trends: PerformanceTrends,
    pub resource_trends: ResourceTrends,

    // Aggregated statistics
    pub statistics: AggregatedStatistics,
}

/// Performance trends over time
#[derive(Debug, Serialize, ToSchema)]
pub struct PerformanceTrends {
    pub qps: Vec<TimeSeriesPoint>,
    pub rps: Vec<TimeSeriesPoint>,
    pub latency_p50: Vec<TimeSeriesPoint>,
    pub latency_p95: Vec<TimeSeriesPoint>,
    pub latency_p99: Vec<TimeSeriesPoint>,
}

/// Resource trends over time
#[derive(Debug, Serialize, ToSchema)]
pub struct ResourceTrends {
    pub cpu_usage: Vec<TimeSeriesPoint>,
    pub memory_usage: Vec<TimeSeriesPoint>,
    pub disk_usage: Vec<TimeSeriesPoint>,
    pub jvm_heap_usage: Vec<TimeSeriesPoint>,
}

/// Time series data point
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

/// Capacity prediction result
#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct CapacityPrediction {
    pub disk_total_bytes: i64,
    pub disk_used_bytes: i64,
    pub disk_usage_pct: f64,
    pub daily_growth_bytes: i64,
    pub days_until_full: Option<i32>,
    pub predicted_full_date: Option<String>,
    pub growth_trend: String,      // "increasing", "stable", "decreasing"
    pub real_data_size_bytes: i64, // Real data size from information_schema (stored in object storage)
}

/// Aggregated statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct AggregatedStatistics {
    pub avg_qps: f64,
    pub max_qps: f64,
    pub avg_latency_p99: f64,
    pub avg_cpu_usage: f64,
    pub avg_memory_usage: f64,
    pub avg_disk_usage: f64,
}

/// Health status card
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthCard {
    pub title: String,
    pub value: String,
    pub status: HealthStatus,
    pub description: String,
}

/// Health status enum
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}

/// Cluster health overview (Hero Card)
#[derive(Debug, Serialize, ToSchema)]
pub struct ClusterHealth {
    pub status: HealthStatus,
    pub score: f64,                // 0-100
    pub starrocks_version: String, // StarRocks version
    pub be_nodes_online: i32,
    pub be_nodes_total: i32,
    pub fe_nodes_online: i32,
    pub fe_nodes_total: i32,
    pub compaction_score: f64,
    pub alerts: Vec<String>,
}

/// Key performance indicators
#[derive(Debug, Serialize, ToSchema)]
pub struct KeyPerformanceIndicators {
    pub qps: f64,
    pub qps_trend: f64, // percentage change
    pub p99_latency_ms: f64,
    pub p99_latency_trend: f64,
    pub success_rate: f64,
    pub success_rate_trend: f64,
    pub error_rate: f64,
}

/// Resource metrics
#[derive(Debug, Serialize, ToSchema)]
pub struct ResourceMetrics {
    pub cpu_usage_pct: f64,
    pub cpu_trend: f64,
    pub memory_usage_pct: f64,
    pub memory_trend: f64,
    pub disk_usage_pct: f64,
    pub disk_trend: f64,
    pub compaction_score: f64,
    pub compaction_status: String, // "normal", "warning", "critical"
}

/// Materialized view statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct MaterializedViewStats {
    pub total: i32,
    pub running: i32,
    pub success: i32,
    pub failed: i32,
    pub pending: i32,
}

/// Load job statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct LoadJobStats {
    pub running: i32,
    pub pending: i32,
    pub finished: i32,
    pub failed: i32,
    pub cancelled: i32,
}

/// Transaction statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionStats {
    pub running: i32,
    pub committed: i32,
    pub aborted: i32,
}

/// Schema change statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct SchemaChangeStats {
    pub running: i32,
    pub pending: i32,
    pub finished: i32,
    pub failed: i32,
    pub cancelled: i32,
}

/// Compaction statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct CompactionStats {
    pub base_compaction_running: i32,
    pub cumulative_compaction_running: i32,
    pub max_score: f64,
    pub avg_score: f64,
    pub be_scores: Vec<BECompactionScore>,
}

/// BE compaction score
#[derive(Debug, Serialize, ToSchema)]
pub struct BECompactionScore {
    pub be_id: i64,
    pub be_host: String,
    pub score: f64,
}

/// Compaction detailed statistics (for storage-compute separation architecture)
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompactionDetailStats {
    pub top_partitions: Vec<TopPartitionByScore>,
    pub task_stats: CompactionTaskStats,
    pub duration_stats: CompactionDurationStats,
}

/// Top partition by compaction score
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopPartitionByScore {
    pub db_name: String,
    pub table_name: String,
    pub partition_name: String,
    pub max_score: f64,
    pub avg_score: f64,
    pub p50_score: f64,
}

/// Compaction task statistics
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompactionTaskStats {
    pub running_count: i32,
    pub finished_count: i32,
    pub total_count: i32,
}

/// Compaction duration statistics
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompactionDurationStats {
    pub min_duration_ms: i64,
    pub max_duration_ms: i64,
    pub avg_duration_ms: i64,
}

/// Session statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct SessionStats {
    pub active_users_1h: i32,
    pub active_users_24h: i32,
    pub current_connections: i32,
    pub running_queries: Vec<RunningQuery>,
}

/// Running query info
#[derive(Debug, Serialize, ToSchema)]
pub struct RunningQuery {
    pub query_id: String,
    pub user: String,
    pub database: String,
    pub start_time: String,
    pub duration_ms: i64,
    pub state: String,
    pub query_preview: String, // First 200 chars
}

/// Network and IO statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkIOStats {
    pub network_tx_bytes_per_sec: f64,
    pub network_rx_bytes_per_sec: f64,
    pub disk_read_bytes_per_sec: f64,
    pub disk_write_bytes_per_sec: f64,
}

/// Alert information
#[derive(Debug, Serialize, ToSchema)]
pub struct Alert {
    pub level: AlertLevel,
    pub category: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub action: Option<String>, // Suggested action
}

/// Alert level
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    Critical,
    Warning,
    Info,
}

/// Extended cluster overview with all modules
#[derive(Debug, Serialize, ToSchema)]
pub struct ExtendedClusterOverview {
    pub cluster_id: i64,
    pub cluster_name: String,
    pub timestamp: DateTime<Utc>,

    // Module 1: Cluster Health (P0)
    pub health: ClusterHealth,

    // Module 2: Key Performance Indicators (P0)
    pub kpi: KeyPerformanceIndicators,

    // Module 3: Resource Metrics (P0)
    pub resources: ResourceMetrics,

    // Module 4-5: Performance & Resource Trends (P0)
    pub performance_trends: PerformanceTrends,
    pub resource_trends: ResourceTrends,

    // Module 6: Data Statistics (P1)
    pub data_stats: Option<DataStatistics>,

    // Module 7: Materialized Views (P1)
    pub mv_stats: MaterializedViewStats,

    // Module 8: Load Jobs (P1)
    pub load_jobs: LoadJobStats,

    // Module 9: Transactions (P1)
    pub transactions: TransactionStats,

    // Module 10: Schema Changes (P1)
    pub schema_changes: SchemaChangeStats,

    // Module 11: Compaction (P1)
    pub compaction: CompactionStats,

    // Module 12: Sessions (P1)
    pub sessions: SessionStats,

    // Module 13: Network & IO (P1)
    pub network_io: NetworkIOStats,

    // Module 17: Capacity Prediction (P2)
    pub capacity: Option<CapacityPrediction>,

    // Module 18: Alerts (P2)
    pub alerts: Vec<Alert>,
}

#[derive(Clone)]
pub struct OverviewService {
    db: SqlitePool,
    cluster_service: Arc<ClusterService>,
    data_statistics_service: Option<Arc<DataStatisticsService>>,
    mysql_pool_manager: Arc<crate::services::mysql_pool_manager::MySQLPoolManager>,
}

impl OverviewService {
    /// Create a new OverviewService
    pub fn new(
        db: SqlitePool,
        cluster_service: Arc<ClusterService>,
        mysql_pool_manager: Arc<crate::services::mysql_pool_manager::MySQLPoolManager>,
    ) -> Self {
        Self { db, cluster_service, data_statistics_service: None, mysql_pool_manager }
    }

    /// Set data statistics service (optional dependency)
    pub fn with_data_statistics(mut self, service: Arc<DataStatisticsService>) -> Self {
        self.data_statistics_service = Some(service);
        self
    }

    /// Get cluster overview (main API)
    pub async fn get_cluster_overview(
        &self,
        cluster_id: i64,
        time_range: TimeRange,
    ) -> ApiResult<ClusterOverview> {
        tracing::debug!(
            "Getting overview for cluster {} with time range {:?}",
            cluster_id,
            time_range
        );

        // Get cluster info
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Get latest snapshot
        let latest_snapshot = self.get_latest_snapshot(cluster_id).await?;

        // Get historical snapshots for trends
        let history = self.get_history_snapshots(cluster_id, &time_range).await?;

        // Calculate trends and statistics
        let performance_trends = self.calculate_performance_trends(&history);
        let resource_trends = self.calculate_resource_trends(&history);
        let statistics = self.calculate_aggregated_statistics(&history);

        Ok(ClusterOverview {
            cluster_id,
            cluster_name: cluster.name,
            timestamp: Utc::now(),
            latest_snapshot,
            performance_trends,
            resource_trends,
            statistics,
        })
    }

    /// Get health status cards
    pub async fn get_health_cards(&self, cluster_id: i64) -> ApiResult<Vec<HealthCard>> {
        let snapshot = self.get_latest_snapshot(cluster_id).await?;

        let snapshot = match snapshot {
            Some(s) => s,
            None => {
                return Ok(vec![HealthCard {
                    title: "No Data".to_string(),
                    value: "N/A".to_string(),
                    status: HealthStatus::Warning,
                    description: "No metrics data available yet".to_string(),
                }]);
            },
        };

        let mut cards = Vec::new();

        // Cluster Status Card
        let cluster_status = if snapshot.backend_alive == snapshot.backend_total
            && snapshot.frontend_alive == snapshot.frontend_total
        {
            HealthStatus::Healthy
        } else if snapshot.backend_alive > 0 && snapshot.frontend_alive > 0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Critical
        };

        cards.push(HealthCard {
            title: "Cluster Status".to_string(),
            value: format!(
                "{}/{} BE/CN, {}/{} FE",
                snapshot.backend_alive,
                snapshot.backend_total,
                snapshot.frontend_alive,
                snapshot.frontend_total
            ),
            status: cluster_status,
            description: "Compute nodes (BE/CN) and Frontend nodes availability".to_string(),
        });

        // QPS Card
        let qps_status = if snapshot.qps < 100.0 {
            HealthStatus::Healthy
        } else if snapshot.qps < 1000.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Critical
        };

        cards.push(HealthCard {
            title: "Query Load".to_string(),
            value: format!("{:.1} QPS", snapshot.qps),
            status: qps_status,
            description: "Current queries per second".to_string(),
        });

        // CPU Usage Card
        let cpu_status = if snapshot.avg_cpu_usage < 70.0 {
            HealthStatus::Healthy
        } else if snapshot.avg_cpu_usage < 85.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Critical
        };

        cards.push(HealthCard {
            title: "CPU Usage".to_string(),
            value: format!("{:.1}%", snapshot.avg_cpu_usage),
            status: cpu_status,
            description: "Average CPU usage across all compute nodes".to_string(),
        });

        // Disk Usage Card
        let disk_status = if snapshot.disk_usage_pct < 70.0 {
            HealthStatus::Healthy
        } else if snapshot.disk_usage_pct < 85.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Critical
        };

        cards.push(HealthCard {
            title: "Disk Usage".to_string(),
            value: format!("{:.1}%", snapshot.disk_usage_pct),
            status: disk_status,
            description: "Total disk space usage".to_string(),
        });

        Ok(cards)
    }

    /// Get performance trends
    pub async fn get_performance_trends(
        &self,
        cluster_id: i64,
        time_range: TimeRange,
    ) -> ApiResult<PerformanceTrends> {
        let history = self.get_history_snapshots(cluster_id, &time_range).await?;
        Ok(self.calculate_performance_trends(&history))
    }

    /// Get resource trends
    pub async fn get_resource_trends(
        &self,
        cluster_id: i64,
        time_range: TimeRange,
    ) -> ApiResult<ResourceTrends> {
        let history = self.get_history_snapshots(cluster_id, &time_range).await?;
        Ok(self.calculate_resource_trends(&history))
    }

    /// Get data statistics (database/table counts, top tables, etc.)
    pub async fn get_data_statistics(
        &self,
        cluster_id: i64,
        time_range: Option<&TimeRange>,
    ) -> ApiResult<DataStatistics> {
        if let Some(ref service) = self.data_statistics_service {
            if let Some(stats) = service.get_statistics(cluster_id).await? {
                let age = Utc::now() - stats.updated_at;
                if age.num_minutes() < 10 {
                    return Ok(stats);
                }
            }

            let time_range_start = time_range.map(|tr| tr.start_time());
            service
                .update_statistics(cluster_id, time_range_start)
                .await
        } else {
            Err(ApiError::internal_error("Data statistics service not configured"))
        }
    }

    /// Predict disk capacity
    ///
    /// Uses linear regression on historical disk usage data to predict when disk will be full
    pub async fn predict_capacity(&self, cluster_id: i64) -> ApiResult<CapacityPrediction> {
        // Get last 2 hours of disk usage data (minimum requirement)
        let cutoff = Utc::now() - chrono::Duration::hours(2);

        let snapshots: Vec<(i64, i64, f64, NaiveDateTime)> = sqlx::query_as(
            r#"
            SELECT 
                disk_total_bytes,
                disk_used_bytes,
                disk_usage_pct,
                collected_at
            FROM metrics_snapshots
            WHERE cluster_id = ? AND collected_at >= ?
            ORDER BY collected_at ASC
            "#,
        )
        .bind(cluster_id)
        .bind(cutoff)
        .fetch_all(&self.db)
        .await?;

        if snapshots.is_empty() {
            return Err(ApiError::internal_error(
                "No historical data available for capacity prediction",
            ));
        }

        // Get latest values
        let latest = snapshots.last().unwrap();
        let disk_total_bytes = latest.0;
        let disk_usage_pct = latest.2;
        // Calculate disk_used_bytes from percentage (as stored value may be 0 in shared-nothing arch)
        let disk_used_bytes = ((disk_total_bytes as f64) * disk_usage_pct / 100.0) as i64;

        // Perform linear regression on disk_used_bytes over time
        // y = disk_used_bytes, x = days since first snapshot
        let first_time = snapshots.first().unwrap().3.and_utc().timestamp();
        let last_time = snapshots.last().unwrap().3.and_utc().timestamp();
        let time_span_days = (last_time - first_time) as f64 / 86400.0;

        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;
        let n = snapshots.len() as f64;

        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for snapshot in &snapshots {
            let x = (snapshot.3.and_utc().timestamp() - first_time) as f64 / 86400.0; // days
            // Calculate used bytes from percentage for each snapshot
            let y = (snapshot.0 as f64) * snapshot.2 / 100.0;

            min_y = min_y.min(y);
            max_y = max_y.max(y);

            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_x2 += x * x;
        }

        // Calculate slope (daily growth rate in bytes)
        let denominator = n * sum_x2 - sum_x * sum_x;
        let daily_growth_bytes = if denominator.abs() < 1e-10 {
            // If denominator is too small, slope is undefined or unstable
            0
        } else {
            let slope = (n * sum_xy - sum_x * sum_y) / denominator;

            // Sanity check: if data variance is too small or slope is unreasonably large, set to 0
            let data_variance = max_y - min_y;
            let slope_abs = slope.abs();

            if data_variance < 1_000_000_000.0 || // < 1GB variance
               slope_abs > 10_000_000_000_000.0 || // > 10TB/day (unreasonable)
               time_span_days < 0.01
            {
                // < 15 minutes (too short for regression)
                tracing::debug!(
                    "Linear regression unstable: variance={:.0}, slope={:.0}, time_span={:.3} days. Setting growth to 0.",
                    data_variance,
                    slope,
                    time_span_days
                );
                0
            } else {
                slope as i64
            }
        };

        // Determine growth trend
        let growth_trend = if daily_growth_bytes > 1_000_000_000 {
            // > 1GB/day
            "increasing"
        } else if daily_growth_bytes > 0 {
            "stable"
        } else {
            "decreasing"
        };

        // Calculate days until full (if disk is growing)
        let (days_until_full, predicted_full_date) = if daily_growth_bytes > 0 {
            let remaining_bytes = disk_total_bytes - disk_used_bytes;
            let days = (remaining_bytes as f64 / daily_growth_bytes as f64).ceil() as i32;

            let full_date = Utc::now() + chrono::Duration::days(days as i64);
            let full_date_str = full_date.format("%Y-%m-%d").to_string();

            (Some(days), Some(full_date_str))
        } else {
            (None, None)
        };

        // Note: real_data_size_bytes will be set in get_extended_overview() from data_stats
        Ok(CapacityPrediction {
            disk_total_bytes,
            disk_used_bytes,
            disk_usage_pct,
            daily_growth_bytes,
            days_until_full,
            predicted_full_date,
            growth_trend: growth_trend.to_string(),
            real_data_size_bytes: 0, // Will be populated from data_stats.total_data_size in get_extended_overview()
        })
    }

    // ========================================
    // Internal helper methods
    // ========================================

    /// Get the latest snapshot for a cluster
    async fn get_latest_snapshot(&self, cluster_id: i64) -> ApiResult<Option<MetricsSnapshot>> {
        #[derive(sqlx::FromRow)]
        struct SnapshotRow {
            cluster_id: i64,
            collected_at: NaiveDateTime,
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

    /// Get historical snapshots for a time range
    async fn get_history_snapshots(
        &self,
        cluster_id: i64,
        time_range: &TimeRange,
    ) -> ApiResult<Vec<MetricsSnapshot>> {
        #[derive(sqlx::FromRow)]
        struct SnapshotRow {
            cluster_id: i64,
            collected_at: NaiveDateTime,
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

        let start_time = time_range.start_time();
        let end_time = time_range.end_time();

        let rows: Vec<SnapshotRow> = sqlx::query_as(
            r#"
            SELECT * FROM metrics_snapshots
            WHERE cluster_id = ? 
              AND collected_at BETWEEN ? AND ?
            ORDER BY collected_at ASC
            "#,
        )
        .bind(cluster_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.db)
        .await?;

        let snapshots = rows
            .into_iter()
            .map(|r| MetricsSnapshot {
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
            })
            .collect();

        Ok(snapshots)
    }

    /// Calculate performance trends from snapshots
    fn calculate_performance_trends(&self, snapshots: &[MetricsSnapshot]) -> PerformanceTrends {
        let qps: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.qps })
            .collect();

        let rps: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.rps })
            .collect();

        let latency_p50: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.query_latency_p50 })
            .collect();

        let latency_p95: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.query_latency_p95 })
            .collect();

        let latency_p99: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.query_latency_p99 })
            .collect();

        PerformanceTrends { qps, rps, latency_p50, latency_p95, latency_p99 }
    }

    /// Calculate resource trends from snapshots
    fn calculate_resource_trends(&self, snapshots: &[MetricsSnapshot]) -> ResourceTrends {
        let cpu_usage: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.avg_cpu_usage })
            .collect();

        let memory_usage: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.avg_memory_usage })
            .collect();

        let disk_usage: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.disk_usage_pct })
            .collect();

        let jvm_heap_usage: Vec<TimeSeriesPoint> = snapshots
            .iter()
            .map(|s| TimeSeriesPoint { timestamp: s.collected_at, value: s.jvm_heap_usage_pct })
            .collect();

        ResourceTrends { cpu_usage, memory_usage, disk_usage, jvm_heap_usage }
    }

    /// Calculate aggregated statistics from snapshots
    fn calculate_aggregated_statistics(
        &self,
        snapshots: &[MetricsSnapshot],
    ) -> AggregatedStatistics {
        if snapshots.is_empty() {
            return AggregatedStatistics {
                avg_qps: 0.0,
                max_qps: 0.0,
                avg_latency_p99: 0.0,
                avg_cpu_usage: 0.0,
                avg_memory_usage: 0.0,
                avg_disk_usage: 0.0,
            };
        }

        let count = snapshots.len() as f64;

        let avg_qps = snapshots.iter().map(|s| s.qps).sum::<f64>() / count;
        let max_qps = snapshots.iter().map(|s| s.qps).fold(0.0, f64::max);
        let avg_latency_p99 = snapshots.iter().map(|s| s.query_latency_p99).sum::<f64>() / count;
        let avg_cpu_usage = snapshots.iter().map(|s| s.avg_cpu_usage).sum::<f64>() / count;
        let avg_memory_usage = snapshots.iter().map(|s| s.avg_memory_usage).sum::<f64>() / count;
        let avg_disk_usage = snapshots.iter().map(|s| s.disk_usage_pct).sum::<f64>() / count;

        AggregatedStatistics {
            avg_qps,
            max_qps,
            avg_latency_p99,
            avg_cpu_usage,
            avg_memory_usage,
            avg_disk_usage,
        }
    }

    /// Get extended cluster overview with all 18 modules
    pub async fn get_extended_overview(
        &self,
        cluster_id: i64,
        time_range: TimeRange,
    ) -> ApiResult<ExtendedClusterOverview> {
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        let (latest, snapshots) = tokio::try_join!(
            self.get_latest_snapshot(cluster_id),
            self.get_history_snapshots(cluster_id, &time_range)
        )?;

        let (
            data_stats_result,
            mv_stats_result,
            load_jobs_result,
            schema_changes_result,
            compaction_result,
            sessions_result,
            capacity_result,
            starrocks_version_result,
        ) = tokio::join!(
            async {
                self.get_data_statistics(cluster_id, Some(&time_range))
                    .await
                    .ok()
            },
            self.get_mv_stats(cluster_id),
            self.get_load_job_stats(cluster_id, &time_range),
            self.get_schema_change_stats(cluster_id, &time_range),
            self.get_compaction_stats(cluster_id),
            self.get_session_stats(cluster_id),
            async {
                match self.predict_capacity(cluster_id).await {
                    Ok(cap) => Some(cap),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to predict capacity for cluster {}: {}",
                            cluster_id,
                            e
                        );
                        None
                    },
                }
            },
            async {
                self.get_starrocks_version(cluster_id)
                    .await
                    .unwrap_or_else(|_| "Unknown".to_string())
            },
        );

        let data_stats = data_stats_result;
        let mv_stats = mv_stats_result?;
        let load_jobs = load_jobs_result?;
        let schema_changes = schema_changes_result?;
        let compaction = compaction_result?;
        let sessions = sessions_result?;
        let mut capacity = capacity_result;
        let starrocks_version = starrocks_version_result;

        let health = if let Some(ref latest_snapshot) = latest {
            self.calculate_cluster_health(cluster_id, latest_snapshot, &starrocks_version)
                .await?
        } else {
            return Err(ApiError::internal_error("No metrics snapshot available"));
        };

        let kpi = self.calculate_kpi(&latest, &snapshots);

        let resources = self.calculate_resource_metrics(&latest, &snapshots);

        let performance_trends = self.calculate_performance_trends(&snapshots);
        let resource_trends = self.calculate_resource_trends(&snapshots);

        let transactions = self.get_transaction_stats(&latest);

        let network_io = self.calculate_network_io_stats(&latest);

        if let (Some(cap), Some(stats)) = (&mut capacity, &data_stats) {
            cap.real_data_size_bytes = stats.total_data_size;
        }

        let alerts = self.generate_alerts(&health, &resources, &compaction);

        Ok(ExtendedClusterOverview {
            cluster_id,
            cluster_name: cluster.name,
            timestamp: Utc::now(),
            health,
            kpi,
            resources,
            performance_trends,
            resource_trends,
            data_stats,
            mv_stats,
            load_jobs,
            transactions,
            schema_changes,
            compaction,
            sessions,
            network_io,
            capacity,
            alerts,
        })
    }

    async fn calculate_cluster_health(
        &self,
        _cluster_id: i64,
        snapshot: &MetricsSnapshot,
        starrocks_version: &str,
    ) -> ApiResult<ClusterHealth> {
        let be_nodes_online = snapshot.backend_alive;
        let be_nodes_total = snapshot.backend_total;
        let fe_nodes_online = snapshot.frontend_alive;
        let fe_nodes_total = snapshot.frontend_total;
        let compaction_score = snapshot.max_compaction_score;

        let mut alerts = Vec::new();
        let status = if be_nodes_online < be_nodes_total {
            alerts.push(format!("{} 计算节点离线", be_nodes_total - be_nodes_online));
            HealthStatus::Critical
        } else if compaction_score > 100.0 {
            alerts.push(format!("Compaction Score过高: {:.1}", compaction_score));
            HealthStatus::Critical
        } else if compaction_score > 50.0 || snapshot.disk_usage_pct > 80.0 {
            if compaction_score > 50.0 {
                alerts.push(format!("Compaction Score偏高: {:.1}", compaction_score));
            }
            if snapshot.disk_usage_pct > 80.0 {
                alerts.push(format!("磁盘使用率偏高: {:.1}%", snapshot.disk_usage_pct));
            }
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        let score: f64 = 100.0
            - (if be_nodes_online < be_nodes_total { 30.0 } else { 0.0 })
            - (if compaction_score > 100.0 {
                20.0
            } else if compaction_score > 50.0 {
                10.0
            } else {
                0.0
            })
            - (if snapshot.disk_usage_pct > 90.0 {
                20.0
            } else if snapshot.disk_usage_pct > 80.0 {
                10.0
            } else {
                0.0
            })
            - (if snapshot.avg_cpu_usage > 90.0 { 10.0 } else { 0.0 });

        Ok(ClusterHealth {
            status,
            score: score.max(0.0),
            starrocks_version: starrocks_version.to_string(),
            be_nodes_online,
            be_nodes_total,
            fe_nodes_online,
            fe_nodes_total,
            compaction_score,
            alerts,
        })
    }

    fn calculate_kpi(
        &self,
        snapshot: &Option<MetricsSnapshot>,
        snapshots: &[MetricsSnapshot],
    ) -> KeyPerformanceIndicators {
        let current = snapshot.as_ref();

        let prev_avg_qps = if snapshots.len() > 1 {
            let prev = &snapshots[0..snapshots.len() - 1];
            prev.iter().map(|s| s.qps).sum::<f64>() / prev.len() as f64
        } else {
            0.0
        };

        let prev_avg_latency = if snapshots.len() > 1 {
            let prev = &snapshots[0..snapshots.len() - 1];
            prev.iter().map(|s| s.query_latency_p99).sum::<f64>() / prev.len() as f64
        } else {
            0.0
        };

        let qps = current.map(|s| s.qps).unwrap_or(0.0);
        let p99_latency_ms = current.map(|s| s.query_latency_p99).unwrap_or(0.0);
        let qps_trend =
            if prev_avg_qps > 0.0 { ((qps - prev_avg_qps) / prev_avg_qps) * 100.0 } else { 0.0 };
        let p99_latency_trend = if prev_avg_latency > 0.0 {
            ((p99_latency_ms - prev_avg_latency) / prev_avg_latency) * 100.0
        } else {
            0.0
        };

        let (success_rate, error_rate) = if let Some(s) = current {
            let total = s.query_total as f64;
            let success = s.query_success as f64;
            let errors = s.query_error as f64;
            if total > 0.0 {
                ((success / total) * 100.0, (errors / total) * 100.0)
            } else {
                (100.0, 0.0)
            }
        } else {
            (100.0, 0.0)
        };

        KeyPerformanceIndicators {
            qps,
            qps_trend,
            p99_latency_ms,
            p99_latency_trend,
            success_rate,
            success_rate_trend: 0.0, // TODO: Calculate from history
            error_rate,
        }
    }

    /// Module 3: Calculate resource metrics
    fn calculate_resource_metrics(
        &self,
        snapshot: &Option<MetricsSnapshot>,
        snapshots: &[MetricsSnapshot],
    ) -> ResourceMetrics {
        let current = snapshot.as_ref();

        // Calculate trends
        let prev_avg_cpu = if snapshots.len() > 1 {
            let prev = &snapshots[0..snapshots.len() - 1];
            prev.iter().map(|s| s.avg_cpu_usage).sum::<f64>() / prev.len() as f64
        } else {
            0.0
        };

        let cpu_usage_pct = current.map(|s| s.avg_cpu_usage).unwrap_or(0.0);
        let memory_usage_pct = current.map(|s| s.avg_memory_usage).unwrap_or(0.0);
        let disk_usage_pct = current.map(|s| s.disk_usage_pct).unwrap_or(0.0);
        let compaction_score = current.map(|s| s.max_compaction_score).unwrap_or(0.0);

        let cpu_trend = if prev_avg_cpu > 0.0 {
            ((cpu_usage_pct - prev_avg_cpu) / prev_avg_cpu) * 100.0
        } else {
            0.0
        };

        let compaction_status = if compaction_score > 100.0 {
            "critical".to_string()
        } else if compaction_score > 50.0 {
            "warning".to_string()
        } else {
            "normal".to_string()
        };

        ResourceMetrics {
            cpu_usage_pct,
            cpu_trend,
            memory_usage_pct,
            memory_trend: 0.0, // TODO: Calculate
            disk_usage_pct,
            disk_trend: 0.0, // TODO: Calculate
            compaction_score,
            compaction_status,
        }
    }

    /// Module 7: Get MV stats from information_schema
    async fn get_mv_stats(&self, cluster_id: i64) -> ApiResult<MaterializedViewStats> {
        // Get cluster info
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Get MySQL connection pool and create client
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        // Query materialized view statistics
        let query = r#"
            SELECT 
                COUNT(*) as total,
                SUM(CASE WHEN is_active = 1 THEN 1 ELSE 0 END) as active,
                SUM(CASE WHEN is_active = 0 THEN 1 ELSE 0 END) as inactive
            FROM information_schema.materialized_views
        "#;

        let (columns, rows) = mysql_client.query_raw(query).await?;

        // Build column index map
        let mut col_idx = std::collections::HashMap::new();
        for (i, col) in columns.iter().enumerate() {
            col_idx.insert(col.clone(), i);
        }

        if let Some(row) = rows.first() {
            let total = col_idx
                .get("total")
                .and_then(|&i| row.get(i))
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);

            let active = col_idx
                .get("active")
                .and_then(|&i| row.get(i))
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);

            Ok(MaterializedViewStats {
                total: total as i32,
                running: 0, // Not available from information_schema
                success: active as i32,
                failed: 0,  // Not available from information_schema
                pending: 0, // Not available from information_schema
            })
        } else {
            Ok(MaterializedViewStats { total: 0, running: 0, success: 0, failed: 0, pending: 0 })
        }
    }

    /// Module 8: Get load job stats from SHOW LOAD
    async fn get_load_job_stats(
        &self,
        cluster_id: i64,
        time_range: &TimeRange,
    ) -> ApiResult<LoadJobStats> {
        // Get cluster info
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Get MySQL connection pool and create client
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        // Calculate time range start time
        let start_time = time_range.start_time();

        // Query load job statistics from information_schema.loads
        // Note: SHOW LOAD returns current database only, so we query information_schema
        let query = format!(
            r#"
            SELECT 
                State,
                COUNT(*) as count
            FROM information_schema.loads
            WHERE CREATE_TIME >= '{}'
            GROUP BY State
            "#,
            start_time.format("%Y-%m-%d %H:%M:%S")
        );

        let (columns, rows) = mysql_client.query_raw(&query).await?;

        // Build column index map
        let mut col_idx = std::collections::HashMap::new();
        for (i, col) in columns.iter().enumerate() {
            col_idx.insert(col.clone(), i);
        }

        let mut stats =
            LoadJobStats { running: 0, pending: 0, finished: 0, failed: 0, cancelled: 0 };

        for row in rows {
            let state = col_idx
                .get("State")
                .or_else(|| col_idx.get("state"))
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_default();

            let count = col_idx
                .get("count")
                .and_then(|&i| row.get(i))
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);

            match state.to_uppercase().as_str() {
                "LOADING" => stats.running = count as i32,
                "PENDING" | "QUEUEING" => stats.pending += count as i32,
                "FINISHED" => stats.finished = count as i32,
                "CANCELLED" => stats.cancelled = count as i32,
                _ => stats.failed += count as i32, // Treat unknown states as failed
            }
        }

        Ok(stats)
    }

    /// Module 9: Get transaction stats
    fn get_transaction_stats(&self, snapshot: &Option<MetricsSnapshot>) -> TransactionStats {
        let snapshot = snapshot.as_ref();
        TransactionStats {
            running: snapshot.map(|s| s.txn_running).unwrap_or(0),
            committed: snapshot.map(|s| s.txn_success_total as i32).unwrap_or(0),
            aborted: snapshot.map(|s| s.txn_failed_total as i32).unwrap_or(0),
        }
    }

    /// Module 10: Get schema change stats by querying audit logs
    /// Tracks ALTER TABLE operations and their status from StarRocks audit logs
    async fn get_schema_change_stats(
        &self,
        cluster_id: i64,
        time_range: &TimeRange,
    ) -> ApiResult<SchemaChangeStats> {
        use crate::services::MySQLClient;

        let cluster = self.cluster_service.get_cluster(cluster_id).await?;
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        // Calculate time range start time
        let start_time = time_range.start_time();

        // Query ALTER TABLE operations from audit logs
        // Track schema changes by analyzing DDL statements in the audit log
        let query = format!(
            r#"
            SELECT 
                queryType,
                state,
                COUNT(*) as count
            FROM starrocks_audit_db__.starrocks_audit_tbl__
            WHERE 
                `timestamp` >= '{}'
                AND queryType LIKE '%ALTER%'
                AND isQuery = 0  -- DDL operations have isQuery = 0
            GROUP BY queryType, state
            "#,
            start_time.format("%Y-%m-%d %H:%M:%S")
        );

        let (columns, rows) = mysql_client.query_raw(&query).await?;

        // Build column index map
        let mut col_idx = std::collections::HashMap::new();
        for (i, col) in columns.iter().enumerate() {
            col_idx.insert(col.clone(), i);
        }

        let mut stats =
            SchemaChangeStats { running: 0, pending: 0, finished: 0, failed: 0, cancelled: 0 };

        // Parse results and aggregate by state
        for row in rows {
            let state = col_idx
                .get("state")
                .and_then(|&i| row.get(i))
                .map(|s| s.to_uppercase())
                .unwrap_or_default();

            let count = col_idx
                .get("count")
                .and_then(|&i| row.get(i))
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);

            match state.as_str() {
                "RUNNING" | "EXECUTING" => stats.running += count,
                "PENDING" | "QUEUEING" => stats.pending += count,
                "FINISHED" | "OK" | "EOF" => stats.finished += count,
                "CANCELLED" | "CANCEL" => stats.cancelled += count,
                "FAILED" | "ERROR" => stats.failed += count,
                _ => {}, // Ignore unknown states
            }
        }

        Ok(stats)
    }

    /// Module 11: Get compaction stats from FE's SHOW PROC '/compactions'
    ///
    /// Note: Compaction Score is calculated at FE level per Partition, not per BE.
    /// Reference: https://forum.mirrorship.cn/t/topic/13256
    async fn get_compaction_stats(&self, cluster_id: i64) -> ApiResult<CompactionStats> {
        use crate::services::MySQLClient;

        // Get cluster info
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Get MySQL connection pool
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let client = MySQLClient::from_pool(pool);

        // Query compaction tasks from FE
        // SHOW PROC '/compactions' shows current running compaction tasks
        let query = "SHOW PROC '/compactions'";
        let (_headers, rows) = client.query_raw(query).await.unwrap_or((vec![], vec![]));

        // Count running compaction tasks
        // Note: In StarRocks shared-data mode, there are no separate
        // base_compaction and cumulative_compaction, just unified compaction tasks
        let total_running = rows.len() as i32;

        // Get max compaction score from metrics snapshot
        // Compaction score is stored in our metrics_snapshots table
        let latest_snapshot = self.get_latest_snapshot(cluster_id).await?;
        let max_score = latest_snapshot
            .as_ref()
            .map(|s| s.max_compaction_score)
            .unwrap_or(0.0);

        // For storage-compute separation mode, we don't track per-BE compaction scores
        // because compaction is scheduled at Partition level by FE
        // We can provide a simplified view based on latest snapshot
        Ok(CompactionStats {
            base_compaction_running: 0, // Not applicable in shared-data mode
            cumulative_compaction_running: total_running, // Total compaction tasks
            max_score,
            avg_score: max_score, // In shared-data, score is per-partition, not per-BE
            be_scores: Vec::new(), // Not applicable - compaction score is per-partition in FE
        })
    }

    /// Get detailed compaction statistics for storage-compute separation architecture
    ///
    /// This method queries:
    /// 1. Top 10 partitions by compaction score from information_schema.partitions_meta
    /// 2. Running and finished compaction tasks from information_schema.be_cloud_native_compactions
    /// 3. Duration statistics (min, max, avg) for compactions within the time range
    pub async fn get_compaction_detail_stats(
        &self,
        cluster_id: i64,
        time_range: &str,
    ) -> ApiResult<CompactionDetailStats> {
        use crate::services::MySQLClient;

        // Get cluster info
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Get MySQL connection pool
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let client = MySQLClient::from_pool(pool);

        // Calculate time filter based on time_range parameter
        let hours_back = match time_range {
            "1h" => 1,
            "6h" => 6,
            "24h" => 24,
            "3d" => 72,
            _ => 1, // default to 1 hour
        };

        // Query 1: Get Top 10 partitions by compaction score
        // Filter out system databases and tables
        let top_partitions_query = r#"
            SELECT 
                DB_NAME, 
                TABLE_NAME, 
                PARTITION_NAME, 
                MAX_CS as max_score, 
                AVG_CS as avg_score, 
                P50_CS as p50_score
            FROM information_schema.partitions_meta
            WHERE MAX_CS > 0 
              AND DB_NAME NOT IN ('_statistics_', 'information_schema', 'sys', 'starrocks_audit_db__')
              AND TABLE_NAME != 'starrocks_audit_tbl__'
            ORDER BY MAX_CS DESC
            LIMIT 10
        "#;

        let (_headers, rows) = client
            .query_raw(top_partitions_query)
            .await
            .unwrap_or((vec![], vec![]));

        let top_partitions: Vec<TopPartitionByScore> = rows
            .into_iter()
            .filter_map(|row| {
                if row.len() >= 6 {
                    let db_name = row.first().map(|s| s.to_string()).unwrap_or_default();
                    let table_name = row.get(1).map(|s| s.to_string()).unwrap_or_default();

                    // Filter out system databases (double protection)
                    if db_name == "_statistics_"
                        || db_name == "information_schema"
                        || db_name == "sys"
                        || db_name == "starrocks_audit_db__"
                        || table_name == "starrocks_audit_tbl__"
                    {
                        tracing::debug!("Filtering out system table: {}.{}", db_name, table_name);
                        return None;
                    }

                    Some(TopPartitionByScore {
                        db_name,
                        table_name,
                        partition_name: row.get(2).map(|s| s.to_string()).unwrap_or_default(),
                        max_score: row.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        avg_score: row.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                        p50_score: row.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    })
                } else {
                    None
                }
            })
            .collect();

        // Query 2: Get task statistics from SHOW PROC '/compactions'
        // Note: SHOW PROC cannot be used in subqueries, so we'll get all data and filter in Rust
        let task_stats_query = r#"SHOW PROC '/compactions'"#;

        let (_headers, rows) = client
            .query_raw(task_stats_query)
            .await
            .unwrap_or((vec![], vec![]));

        tracing::debug!("Compaction PROC query returned {} rows", rows.len());

        // Process compaction data in Rust since SHOW PROC cannot be used in subqueries
        let mut total_count = 0;
        let mut running_count = 0;
        let mut finished_count = 0;
        let mut durations: Vec<i64> = Vec::new();

        for row in &rows {
            if row.len() >= 5 {
                // Parse StartTime and FinishTime
                let start_time_str = row.get(2).map(|s| s.to_string()).unwrap_or_default();
                let finish_time_str = row.get(4).map(|s| s.to_string()).unwrap_or_default();

                // Check if task is within time range or still running
                let is_within_time_range = if !start_time_str.is_empty() && start_time_str != "NULL"
                {
                    // Parse start time and check if within range
                    if let Ok(start_time) =
                        chrono::NaiveDateTime::parse_from_str(&start_time_str, "%Y-%m-%d %H:%M:%S")
                    {
                        let now = chrono::Utc::now().naive_utc();
                        let time_diff = now.signed_duration_since(start_time);
                        time_diff.num_hours() <= hours_back
                    } else {
                        false
                    }
                } else {
                    false
                };

                let is_running = finish_time_str.is_empty() || finish_time_str == "NULL";
                let _is_finished = !is_running;

                if is_within_time_range || is_running {
                    total_count += 1;
                    if is_running {
                        running_count += 1;
                    } else {
                        finished_count += 1;

                        // Calculate duration for finished tasks
                        if !start_time_str.is_empty()
                            && start_time_str != "NULL"
                            && !finish_time_str.is_empty()
                            && finish_time_str != "NULL"
                            && let (Ok(start_time), Ok(finish_time)) = (
                                chrono::NaiveDateTime::parse_from_str(
                                    &start_time_str,
                                    "%Y-%m-%d %H:%M:%S",
                                ),
                                chrono::NaiveDateTime::parse_from_str(
                                    &finish_time_str,
                                    "%Y-%m-%d %H:%M:%S",
                                ),
                            )
                        {
                            let duration =
                                finish_time.signed_duration_since(start_time).num_seconds();
                            durations.push(duration);
                        }
                    }
                }
            }
        }

        let task_stats = CompactionTaskStats { total_count, running_count, finished_count };

        tracing::debug!(
            "Processed compaction stats: total={}, running={}, finished={}",
            total_count,
            running_count,
            finished_count
        );

        // Calculate duration statistics from the durations we collected
        let duration_stats = if durations.is_empty() {
            CompactionDurationStats { min_duration_ms: 0, max_duration_ms: 0, avg_duration_ms: 0 }
        } else {
            let min_duration = durations.iter().min().unwrap_or(&0);
            let max_duration = durations.iter().max().unwrap_or(&0);
            let avg_duration = durations.iter().sum::<i64>() / durations.len() as i64;

            CompactionDurationStats {
                min_duration_ms: min_duration * 1000,
                max_duration_ms: max_duration * 1000,
                avg_duration_ms: avg_duration * 1000,
            }
        };

        tracing::debug!(
            "Duration stats: min={}ms, max={}ms, avg={}ms",
            duration_stats.min_duration_ms,
            duration_stats.max_duration_ms,
            duration_stats.avg_duration_ms
        );

        Ok(CompactionDetailStats { top_partitions, task_stats, duration_stats })
    }

    /// Module 12: Get session stats from SHOW PROCESSLIST
    async fn get_session_stats(&self, cluster_id: i64) -> ApiResult<SessionStats> {
        use crate::services::MySQLClient;
        use chrono::Utc;

        // Get cluster info
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Get MySQL connection pool
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let client = MySQLClient::from_pool(pool);

        // Query SHOW PROCESSLIST to get current connections
        let query = "SHOW FULL PROCESSLIST";
        let (_headers, rows) = client.query_raw(query).await?;

        let current_connections = rows.len() as i32;

        // Parse running queries (State = 'Query' and Time > 0)
        let mut running_queries = Vec::new();

        for row in &rows {
            if row.len() >= 8 {
                let state = row.get(4).map(|s| s.as_str()).unwrap_or("");
                let time_str = row.get(5).map(|s| s.as_str()).unwrap_or("0");
                let info = row.get(7).map(|s| s.as_str()).unwrap_or("");

                // Only include queries that are actively running
                if state == "Query" && !info.is_empty() {
                    let time_secs = time_str.parse::<i64>().unwrap_or(0);

                    // Skip internal queries and very short queries
                    if time_secs > 1 && !info.starts_with("SHOW") {
                        let query_id = row.first().map(|s| s.to_string()).unwrap_or_default();
                        let user = row.get(1).map(|s| s.to_string()).unwrap_or_default();
                        let db = row.get(3).map(|s| s.to_string()).unwrap_or_default();

                        running_queries.push(RunningQuery {
                            query_id,
                            user,
                            database: db,
                            start_time: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                            duration_ms: time_secs * 1000,
                            state: state.to_string(),
                            query_preview: info.chars().take(200).collect(),
                        });
                    }
                }
            }
        }

        // Sort by duration (longest first) and limit to top 10
        running_queries.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));
        running_queries.truncate(10);

        // Get active users from audit logs if available (simplified - use data statistics service)
        let (active_users_1h, active_users_24h) =
            if let Some(service) = &self.data_statistics_service {
                match service.get_statistics(cluster_id).await {
                    Ok(Some(stats)) => (stats.active_users_1h, stats.active_users_24h),
                    _ => (0, 0),
                }
            } else {
                (0, 0)
            };

        Ok(SessionStats { active_users_1h, active_users_24h, current_connections, running_queries })
    }

    /// Module 13: Calculate network & IO stats
    fn calculate_network_io_stats(&self, snapshot: &Option<MetricsSnapshot>) -> NetworkIOStats {
        let snapshot = snapshot.as_ref();
        NetworkIOStats {
            network_tx_bytes_per_sec: snapshot.map(|s| s.network_send_rate).unwrap_or(0.0),
            network_rx_bytes_per_sec: snapshot.map(|s| s.network_receive_rate).unwrap_or(0.0),
            disk_read_bytes_per_sec: snapshot.map(|s| s.io_read_rate).unwrap_or(0.0),
            disk_write_bytes_per_sec: snapshot.map(|s| s.io_write_rate).unwrap_or(0.0),
        }
    }

    /// Module 18: Generate alerts based on current state
    fn generate_alerts(
        &self,
        health: &ClusterHealth,
        resources: &ResourceMetrics,
        _compaction: &CompactionStats,
    ) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // Critical: Node offline
        if health.be_nodes_online < health.be_nodes_total {
            alerts.push(Alert {
                level: AlertLevel::Critical,
                category: "节点状态".to_string(),
                message: format!("{} 计算节点离线", health.be_nodes_total - health.be_nodes_online),
                timestamp: Utc::now(),
                action: Some("检查计算节点状态并重启".to_string()),
            });
        }

        // Critical: Compaction score too high
        if health.compaction_score > 100.0 {
            alerts.push(Alert {
                level: AlertLevel::Critical,
                category: "Compaction".to_string(),
                message: format!("Compaction Score过高: {:.1}", health.compaction_score),
                timestamp: Utc::now(),
                action: Some("检查磁盘IO性能，考虑增加计算节点".to_string()),
            });
        }

        // Warning: High disk usage
        if resources.disk_usage_pct > 80.0 {
            let level = if resources.disk_usage_pct > 90.0 {
                AlertLevel::Critical
            } else {
                AlertLevel::Warning
            };
            alerts.push(Alert {
                level,
                category: "容量".to_string(),
                message: format!("磁盘使用率过高: {:.1}%", resources.disk_usage_pct),
                timestamp: Utc::now(),
                action: Some("清理过期数据或扩容磁盘".to_string()),
            });
        }

        // Warning: High CPU usage
        if resources.cpu_usage_pct > 80.0 {
            alerts.push(Alert {
                level: AlertLevel::Warning,
                category: "资源".to_string(),
                message: format!("CPU使用率偏高: {:.1}%", resources.cpu_usage_pct),
                timestamp: Utc::now(),
                action: Some("检查慢查询，优化查询性能".to_string()),
            });
        }

        alerts
    }

    async fn get_starrocks_version(&self, cluster_id: i64) -> ApiResult<String> {
        use crate::services::StarRocksClient;
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;
        let starrocks_client = StarRocksClient::new(cluster, self.mysql_pool_manager.clone());
        let frontends = starrocks_client.get_frontends().await?;
        if let Some(fe) = frontends.first() {
            Ok(fe.version.clone())
        } else {
            Ok("Unknown".to_string())
        }
    }
}
