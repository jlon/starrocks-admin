use axum::{extract::{Path, State}, Json};
use std::sync::Arc;

use crate::AppState;
use crate::models::MetricsSummary;
use crate::services::StarRocksClient;
use crate::utils::ApiResult;

// Get metrics summary for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/metrics/summary",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Metrics summary", body = MetricsSummary),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Monitoring"
)]
pub async fn get_metrics_summary(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<MetricsSummary>> {
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    let client = StarRocksClient::new(cluster);

    // Fetch all required data
    let metrics_text = client.get_metrics().await?;
    let metrics_map = client.parse_prometheus_metrics(&metrics_text)?;
    let backends = client.get_backends().await?;
    let runtime_info = client.get_runtime_info().await?;

    // Aggregate backend metrics
    let backend_total = backends.len();
    let backend_alive = backends.iter().filter(|b| b.alive == "true").count();
    
    let tablet_count: i64 = backends
        .iter()
        .filter_map(|b| b.tablet_num.parse::<i64>().ok())
        .sum();

    let total_running_queries: i32 = backends
        .iter()
        .filter_map(|b| b.num_running_queries.parse::<i32>().ok())
        .sum();

    let avg_cpu_usage_pct = if backend_total > 0 {
        backends
            .iter()
            .filter_map(|b| b.cpu_used_pct.trim_end_matches('%').parse::<f64>().ok())
            .sum::<f64>()
            / backend_total as f64
    } else {
        0.0
    };

    let avg_mem_usage_pct = if backend_total > 0 {
        backends
            .iter()
            .filter_map(|b| b.mem_used_pct.trim_end_matches('%').parse::<f64>().ok())
            .sum::<f64>()
            / backend_total as f64
    } else {
        0.0
    };

    // Parse storage sizes (simplified - in production, handle units properly)
    let disk_used_bytes: u64 = backends
        .iter()
        .filter_map(|b| parse_storage_size(&b.data_used_capacity))
        .sum();

    let disk_total_bytes: u64 = backends
        .iter()
        .filter_map(|b| parse_storage_size(&b.total_capacity))
        .sum();

    let disk_usage_pct = if disk_total_bytes > 0 {
        (disk_used_bytes as f64 / disk_total_bytes as f64) * 100.0
    } else {
        0.0
    };

    // Calculate JVM metrics
    let jvm_heap_used = runtime_info.total_mem - runtime_info.free_mem;
    let jvm_heap_usage_pct = if runtime_info.total_mem > 0 {
        (jvm_heap_used as f64 / runtime_info.total_mem as f64) * 100.0
    } else {
        0.0
    };

    // Build metrics summary
    let summary = MetricsSummary {
        // Query metrics from Prometheus
        qps: metrics_map.get("starrocks_fe_qps").copied().unwrap_or(0.0),
        rps: metrics_map.get("starrocks_fe_rps").copied().unwrap_or(0.0),
        query_total: metrics_map.get("starrocks_fe_query_total").copied().unwrap_or(0.0) as i64,
        query_success: metrics_map.get("starrocks_fe_query_success").copied().unwrap_or(0.0) as i64,
        query_err: metrics_map.get("starrocks_fe_query_err").copied().unwrap_or(0.0) as i64,
        query_timeout: metrics_map.get("starrocks_fe_query_timeout").copied().unwrap_or(0.0) as i64,
        query_err_rate: metrics_map.get("starrocks_fe_query_err_rate").copied().unwrap_or(0.0),
        query_latency_p50: metrics_map.get("starrocks_fe_query_latency").copied().unwrap_or(0.0),
        query_latency_p95: metrics_map.get("starrocks_fe_query_latency").copied().unwrap_or(0.0),
        query_latency_p99: metrics_map.get("starrocks_fe_query_latency").copied().unwrap_or(0.0),

        // FE system metrics
        jvm_heap_total: runtime_info.total_mem,
        jvm_heap_used,
        jvm_heap_usage_pct,
        jvm_thread_count: runtime_info.thread_cnt,

        // Backend aggregate metrics
        backend_total,
        backend_alive,
        tablet_count,
        disk_total_bytes,
        disk_used_bytes,
        disk_usage_pct,
        avg_cpu_usage_pct,
        avg_mem_usage_pct,
        total_running_queries,

        // Storage metrics
        max_compaction_score: metrics_map
            .get("starrocks_fe_max_tablet_compaction_score")
            .copied()
            .unwrap_or(0.0),

        // Transaction metrics
        txn_begin: metrics_map.get("starrocks_fe_txn_begin").copied().unwrap_or(0.0) as i64,
        txn_success: metrics_map.get("starrocks_fe_txn_success").copied().unwrap_or(0.0) as i64,
        txn_failed: metrics_map.get("starrocks_fe_txn_failed").copied().unwrap_or(0.0) as i64,

        // Load metrics
        load_finished: metrics_map.get("starrocks_fe_load_finished").copied().unwrap_or(0.0) as i64,
        routine_load_rows: metrics_map
            .get("starrocks_fe_routine_load_rows")
            .copied()
            .unwrap_or(0.0) as i64,
    };

    Ok(Json(summary))
}

// Parse storage size string like "1.5 TB", "500 GB", etc.
fn parse_storage_size(size_str: &str) -> Option<u64> {
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

    Some(bytes as u64)
}

