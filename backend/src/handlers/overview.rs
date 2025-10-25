// Cluster Overview Handlers
// Purpose: HTTP API endpoints for cluster overview functionality
// Design Ref: ARCHITECTURE_ANALYSIS_AND_INTEGRATION.md

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;
use crate::services::{
    AuditLogService, CapacityPrediction, ClusterOverview, DataStatistics, HealthCard, PerformanceTrends, ResourceTrends, SlowQuery, TimeRange,
};
use crate::utils::ApiResult;

/// Query parameters for overview endpoints
#[derive(Debug, Deserialize)]
pub struct OverviewQueryParams {
    #[serde(default = "default_time_range")]
    pub time_range: TimeRange,
}

fn default_time_range() -> TimeRange {
    TimeRange::Hours24
}

/// Query parameters for trend endpoints
#[derive(Debug, Deserialize)]
pub struct TrendQueryParams {
    #[serde(default = "default_time_range")]
    pub time_range: TimeRange,
}

// ========================================
// API Handlers
// ========================================

/// Get cluster overview
/// 
/// Returns comprehensive cluster overview including:
/// - Latest metrics snapshot
/// - Performance trends (QPS, latency, error rate)
/// - Resource trends (CPU, memory, disk)
/// - Aggregated statistics
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/overview",
    params(
        ("id" = i64, Path, description = "Cluster ID"),
        ("time_range" = Option<String>, Query, description = "Time range: 1h, 6h, 24h, 3d (default: 24h)")
    ),
    responses(
        (status = 200, description = "Cluster overview data", body = ClusterOverview),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Cluster Overview"
)]
pub async fn get_cluster_overview(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
    Query(params): Query<OverviewQueryParams>,
) -> ApiResult<Json<ClusterOverview>> {
    tracing::debug!(
        "GET /api/clusters/{}/overview?time_range={:?}",
        cluster_id,
        params.time_range
    );

    let overview = state.overview_service
        .get_cluster_overview(cluster_id, params.time_range)
        .await?;

    Ok(Json(overview))
}

/// Get health status cards
/// 
/// Returns health status cards for quick overview:
/// - Cluster status (BE/FE availability)
/// - Query load (QPS)
/// - CPU usage
/// - Disk usage
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/overview/health",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Health status cards", body = Vec<HealthCard>),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Cluster Overview"
)]
pub async fn get_health_cards(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<Vec<HealthCard>>> {
    tracing::debug!("GET /api/clusters/{}/overview/health", cluster_id);

    let cards = state.overview_service.get_health_cards(cluster_id).await?;

    Ok(Json(cards))
}

/// Get performance trends
/// 
/// Returns time series data for performance metrics:
/// - QPS (Queries per second)
/// - Latency P99
/// - Error rate
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/overview/performance",
    params(
        ("id" = i64, Path, description = "Cluster ID"),
        ("time_range" = Option<String>, Query, description = "Time range: 1h, 6h, 24h, 3d (default: 24h)")
    ),
    responses(
        (status = 200, description = "Performance trends", body = PerformanceTrends),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Cluster Overview"
)]
pub async fn get_performance_trends(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
    Query(params): Query<TrendQueryParams>,
) -> ApiResult<Json<PerformanceTrends>> {
    tracing::debug!(
        "GET /api/clusters/{}/overview/performance?time_range={:?}",
        cluster_id,
        params.time_range
    );

    let trends = overview_service
        .get_performance_trends(cluster_id, params.time_range)
        .await?;

    Ok(Json(trends))
}

/// Get resource trends
/// 
/// Returns time series data for resource usage:
/// - CPU usage
/// - Memory usage
/// - Disk usage
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/overview/resources",
    params(
        ("id" = i64, Path, description = "Cluster ID"),
        ("time_range" = Option<String>, Query, description = "Time range: 1h, 6h, 24h, 3d (default: 24h)")
    ),
    responses(
        (status = 200, description = "Resource trends", body = ResourceTrends),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Cluster Overview"
)]
pub async fn get_resource_trends(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
    Query(params): Query<TrendQueryParams>,
) -> ApiResult<Json<ResourceTrends>> {
    tracing::debug!(
        "GET /api/clusters/{}/overview/resources?time_range={:?}",
        cluster_id,
        params.time_range
    );

    let trends = overview_service
        .get_resource_trends(cluster_id, params.time_range)
        .await?;

    Ok(Json(trends))
}

/// Get data statistics
/// 
/// Returns cached data statistics including:
/// - Database and table counts
/// - Top 20 tables by size
/// - Top 20 tables by access count
/// - Materialized view statistics
/// - Schema change statistics
/// - Active users
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/overview/data-stats",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Data statistics", body = DataStatistics),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Cluster Overview"
)]
pub async fn get_data_statistics(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<DataStatistics>> {
    tracing::debug!("GET /api/clusters/{}/overview/data-stats", cluster_id);

    let stats = state.overview_service.get_data_statistics(cluster_id).await?;

    Ok(Json(stats))
}

/// Get slow queries
/// 
/// Returns slow-running queries from audit logs
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/overview/slow-queries",
    params(
        ("id" = i64, Path, description = "Cluster ID"),
        ("hours" = Option<i32>, Query, description = "Time window in hours (default: 24)"),
        ("min_duration" = Option<i64>, Query, description = "Minimum duration in ms (default: 1000)"),
        ("limit" = Option<usize>, Query, description = "Maximum results (default: 20)")
    ),
    responses(
        (status = 200, description = "Slow queries list", body = Vec<SlowQuery>),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Cluster Overview"
)]
pub async fn get_slow_queries(
    State(state): State<Arc<crate::AppState>>,
    Path(cluster_id): Path<i64>,
    Query(params): Query<SlowQueryParams>,
) -> ApiResult<Json<Vec<SlowQuery>>> {
    tracing::debug!("GET /api/clusters/{}/overview/slow-queries", cluster_id);

    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    
    let audit_service = AuditLogService::new(Arc::clone(&state.mysql_pool_manager));
    let slow_queries = audit_service
        .get_slow_queries(
            &cluster,
            params.hours.unwrap_or(24),
            params.min_duration.unwrap_or(1000),
            params.limit.unwrap_or(20),
        )
        .await?;

    Ok(Json(slow_queries))
}

#[derive(Debug, Deserialize)]
pub struct SlowQueryParams {
    pub hours: Option<i32>,
    pub min_duration: Option<i64>,
    pub limit: Option<usize>,
}

/// Get capacity prediction
/// 
/// Returns disk capacity prediction based on historical growth trend
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/overview/capacity-prediction",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Capacity prediction", body = CapacityPrediction),
        (status = 404, description = "Cluster not found or no historical data"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Cluster Overview"
)]
pub async fn get_capacity_prediction(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<CapacityPrediction>> {
    tracing::debug!("GET /api/clusters/{}/overview/capacity-prediction", cluster_id);

    let prediction = state.overview_service.predict_capacity(cluster_id).await?;

    Ok(Json(prediction))
}

