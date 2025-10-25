use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::AppState;
use crate::models::{
    ClusterHealth, ClusterResponse, CreateClusterRequest, UpdateClusterRequest,
};
use crate::utils::ApiResult;
use serde::Deserialize;

// Create a new cluster
#[utoipa::path(
    post,
    path = "/api/clusters",
    request_body = CreateClusterRequest,
    responses(
        (status = 200, description = "Cluster created successfully", body = ClusterResponse),
        (status = 400, description = "Bad request")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn create_cluster(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user_id): axum::extract::Extension<i64>,
    Json(req): Json<CreateClusterRequest>,
) -> ApiResult<Json<ClusterResponse>> {
    tracing::info!("Cluster creation request: name={}, host={}", req.name, req.fe_host);
    tracing::debug!("Cluster creation details: user_id={}, port={}, ssl={}", 
                   user_id, req.fe_http_port, req.enable_ssl);
    
    let cluster = state.cluster_service.create_cluster(req, user_id).await?;
    
    tracing::info!("Cluster created successfully: {} (ID: {})", cluster.name, cluster.id);
    Ok(Json(cluster.into()))
}

// List all clusters
#[utoipa::path(
    get,
    path = "/api/clusters",
    responses(
        (status = 200, description = "List of clusters", body = Vec<ClusterResponse>)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn list_clusters(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ClusterResponse>>> {
    tracing::debug!("Listing all clusters");
    
    let clusters = state.cluster_service.list_clusters().await?;
    let responses: Vec<ClusterResponse> = clusters.into_iter().map(|c| c.into()).collect();
    
    tracing::debug!("Retrieved {} clusters", responses.len());
    Ok(Json(responses))
}

// Get cluster by ID
#[utoipa::path(
    get,
    path = "/api/clusters/{id}",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Cluster details", body = ClusterResponse),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn get_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<ClusterResponse>> {
    let cluster = state.cluster_service.get_cluster(id).await?;
    Ok(Json(cluster.into()))
}

// Update cluster
#[utoipa::path(
    put,
    path = "/api/clusters/{id}",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    request_body = UpdateClusterRequest,
    responses(
        (status = 200, description = "Cluster updated successfully", body = ClusterResponse),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn update_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateClusterRequest>,
) -> ApiResult<Json<ClusterResponse>> {
    let cluster = state.cluster_service.update_cluster(id, req).await?;
    Ok(Json(cluster.into()))
}

// Delete cluster
#[utoipa::path(
    delete,
    path = "/api/clusters/{id}",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Cluster deleted successfully"),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn delete_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    tracing::warn!("Cluster deletion request for ID: {}", id);
    
    state.cluster_service.delete_cluster(id).await?;
    
    tracing::warn!("Cluster deleted successfully: ID {}", id);
    Ok(Json(serde_json::json!({"message": "Cluster deleted successfully"})))
}

// Health check request (optional body for new cluster mode)
#[derive(Deserialize)]
pub struct HealthCheckRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe_http_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe_query_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub enable_ssl: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
}

// Get cluster health
// Supports two modes:
// 1. GET with cluster ID: Check health of existing cluster from database
// 2. POST with connection details: Check health with provided credentials (for new cluster testing)
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/health",
    params(
        ("id" = i64, Path, description = "Cluster ID (use 0 for new cluster test with POST)")
    ),
    request_body = HealthCheckRequest,
    responses(
        (status = 200, description = "Cluster health status", body = ClusterHealth),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn get_cluster_health(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<i64>,
    body: Option<Json<HealthCheckRequest>>,
) -> ApiResult<Json<ClusterHealth>> {
    use crate::models::Cluster;
    
    // Mode 1: If body provided with connection details, use them (new cluster mode)
    if let Some(Json(health_req)) = body
        && health_req.fe_host.is_some() {
            tracing::info!("Health check with provided credentials: host={}", 
                         health_req.fe_host.as_ref().unwrap());
            tracing::debug!("Health check details: port={}, ssl={}, catalog={}", 
                          health_req.fe_http_port.unwrap_or(8030), 
                          health_req.enable_ssl, 
                          health_req.catalog.as_deref().unwrap_or("default_catalog"));
            
            let temp_cluster = Cluster {
                id: 0,
                name: "test".to_string(),
                description: None,
                fe_host: health_req.fe_host.unwrap(),
                fe_http_port: health_req.fe_http_port.unwrap_or(8030),
                fe_query_port: health_req.fe_query_port.unwrap_or(9030),
                username: health_req.username.unwrap_or_else(|| "root".to_string()),
                password_encrypted: health_req.password.unwrap_or_default(),
                enable_ssl: health_req.enable_ssl,
                connection_timeout: 10,
                catalog: health_req.catalog.unwrap_or_else(|| "default_catalog".to_string()),
                tags: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                created_by: None,
            };
            
            let health = state.cluster_service.get_cluster_health_for_cluster(&temp_cluster, &state.mysql_pool_manager).await?;
            return Ok(Json(health));
    }
    
    // Mode 2: Use cluster ID (existing cluster mode)
    tracing::info!("Health check for existing cluster ID: {}", id);
    let health = state.cluster_service.get_cluster_health(id).await?;
    
    tracing::debug!("Health check result: status={:?}, checks={:?}", 
                   health.status, health.checks);
    
    Ok(Json(health))
}

