use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::AppState;
use crate::models::{ClusterHealth, ClusterResponse, CreateClusterRequest, UpdateClusterRequest};
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(req): Json<CreateClusterRequest>,
) -> ApiResult<Json<ClusterResponse>> {
    tracing::info!(
        "Cluster creation request: name={}, host={} by user {} (org: {:?}, super_admin: {})",
        req.name,
        req.fe_host,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    tracing::debug!(
        "Cluster creation details: user_id={}, port={}, ssl={}",
        org_ctx.user_id,
        req.fe_http_port,
        req.enable_ssl
    );

    let cluster = state
        .cluster_service
        .create_cluster(req, org_ctx.user_id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<Vec<ClusterResponse>>> {
    tracing::debug!(
        "Listing clusters for user {} (org: {:?}, super_admin: {})",
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );

    let clusters = state.cluster_service.list_clusters().await?;
    let filtered = if org_ctx.is_super_admin {
        clusters
    } else {
        clusters
            .into_iter()
            .filter(|c| c.organization_id == org_ctx.organization_id)
            .collect()
    };
    let responses: Vec<ClusterResponse> = filtered.into_iter().map(|c| c.into()).collect();

    tracing::debug!("Retrieved {} clusters for user {}", responses.len(), org_ctx.user_id);
    Ok(Json(responses))
}

// Get the currently active cluster
#[utoipa::path(
    get,
    path = "/api/clusters/active",
    responses(
        (status = 200, description = "Active cluster details", body = ClusterResponse),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn get_active_cluster(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<ClusterResponse>> {
    tracing::debug!(
        "Getting active cluster for user {} (org: {:?}, super_admin: {})",
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );

    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    tracing::debug!(
        "Active cluster for user {}: {} (ID: {})",
        org_ctx.user_id,
        cluster.name,
        cluster.id
    );
    Ok(Json(cluster.into()))
}

// Set a cluster as active
#[utoipa::path(
    put,
    path = "/api/clusters/{id}/activate",
    params(
        ("id" = i64, Path, description = "Cluster ID to activate")
    ),
    responses(
        (status = 200, description = "Cluster activated successfully", body = ClusterResponse),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn activate_cluster(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<ClusterResponse>> {
    tracing::info!(
        "Activating cluster: ID {} for user {} (org: {:?}, super_admin: {})",
        id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );

    // Verify user can activate this cluster (org scope or super admin)
    let target = state.cluster_service.get_cluster(id).await?;
    if !org_ctx.is_super_admin && target.organization_id != org_ctx.organization_id {
        return Err(crate::utils::ApiError::forbidden(
            "You can only activate clusters in your organization",
        ));
    }

    let cluster = state.cluster_service.set_active_cluster(id).await?;

    tracing::info!(
        "Cluster activated successfully: {} (ID: {}) by user {}",
        cluster.name,
        cluster.id,
        org_ctx.user_id
    );
    Ok(Json(cluster.into()))
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<ClusterResponse>> {
    let cluster = state.cluster_service.get_cluster(id).await?;
    if !org_ctx.is_super_admin && cluster.organization_id != org_ctx.organization_id {
        return Err(crate::utils::ApiError::forbidden(
            "You can only view clusters within your organization",
        ));
    }
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(req): Json<UpdateClusterRequest>,
) -> ApiResult<Json<ClusterResponse>> {
    let existing = state.cluster_service.get_cluster(id).await?;
    if !org_ctx.is_super_admin && existing.organization_id != org_ctx.organization_id {
        return Err(crate::utils::ApiError::forbidden(
            "You can only update clusters within your organization",
        ));
    }
    // Only check if organization_id is being changed (not just present)
    if !org_ctx.is_super_admin
        && req.organization_id.is_some()
        && req.organization_id != existing.organization_id
    {
        return Err(crate::utils::ApiError::forbidden(
            "Only super administrators can reassign cluster organization",
        ));
    }

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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<serde_json::Value>> {
    tracing::warn!("Cluster deletion request for ID: {}", id);

    let existing = state.cluster_service.get_cluster(id).await?;
    if !org_ctx.is_super_admin && existing.organization_id != org_ctx.organization_id {
        return Err(crate::utils::ApiError::forbidden(
            "You can only delete clusters within your organization",
        ));
    }

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

    if let Some(Json(health_req)) = body
        && health_req.fe_host.is_some()
    {
        let fe_host = health_req
            .fe_host
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let username = health_req
            .username
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let password = health_req.password.as_ref().map(|s| s.to_string());

        tracing::info!(
            "Health check with provided credentials: host={}",
            fe_host.as_deref().unwrap_or("unknown")
        );
        tracing::debug!(
            "Health check details: port={}, ssl={}, catalog={}",
            health_req.fe_http_port.unwrap_or(8030),
            health_req.enable_ssl,
            health_req.catalog.as_deref().unwrap_or("default_catalog")
        );

        if fe_host.is_none() {
            return Err(crate::utils::ApiError::validation_error(
                "Missing required field: fe_host",
            ));
        }

        let temp_cluster = Cluster {
            id: 0,
            name: "test".to_string(),
            description: None,
            fe_host: fe_host.unwrap(),
            fe_http_port: health_req.fe_http_port.unwrap_or(8030),
            fe_query_port: health_req.fe_query_port.unwrap_or(9030),
            username: username.unwrap_or_else(|| "root".to_string()),
            password_encrypted: password.unwrap_or_default(),
            enable_ssl: health_req.enable_ssl,
            connection_timeout: 10,
            catalog: health_req
                .catalog
                .unwrap_or_else(|| "default_catalog".to_string()),
            is_active: false,
            tags: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            created_by: None,
            organization_id: None,
            deployment_mode: crate::models::cluster::DeploymentMode::default(),
        };

        let health = state
            .cluster_service
            .get_cluster_health_for_cluster(&temp_cluster)
            .await?;
        return Ok(Json(health));
    }

    // Mode 2: Use cluster ID (existing cluster mode)
    tracing::info!("Health check for existing cluster ID: {}", id);
    let health = state.cluster_service.get_cluster_health(id).await?;

    tracing::debug!("Health check result: status={:?}, checks={:?}", health.status, health.checks);

    Ok(Json(health))
}

/// Test cluster connection with provided credentials (no ID required)
#[utoipa::path(
    post,
    path = "/api/clusters/health/test",
    request_body = HealthCheckRequest,
    responses(
        (status = 200, description = "Connection test result", body = ClusterHealth),
        (status = 400, description = "Invalid request"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Clusters"
)]
pub async fn test_cluster_connection(
    State(state): State<Arc<crate::AppState>>,
    Json(health_req): Json<HealthCheckRequest>,
) -> ApiResult<Json<ClusterHealth>> {
    use crate::models::Cluster;

    // Normalize inputs to avoid trailing/leading spaces causing DNS failures
    let fe_host = health_req
        .fe_host
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let username = health_req
        .username
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let password = health_req.password.as_ref().map(|s| s.to_string());

    tracing::info!(
        "Testing connection with provided credentials: host={}",
        fe_host.as_deref().unwrap_or("unknown")
    );

    // Validate required fields
    if fe_host.is_none() {
        return Err(crate::utils::ApiError::validation_error("Missing required field: fe_host"));
    }

    let temp_cluster = Cluster {
        id: 0,
        name: "test".to_string(),
        description: None,
        fe_host: fe_host.unwrap(),
        fe_http_port: health_req.fe_http_port.unwrap_or(8030),
        fe_query_port: health_req.fe_query_port.unwrap_or(9030),
        username: username.unwrap_or_else(|| "root".to_string()),
        password_encrypted: password.unwrap_or_default(),
        enable_ssl: health_req.enable_ssl,
        connection_timeout: 10,
        catalog: health_req
            .catalog
            .unwrap_or_else(|| "default_catalog".to_string()),
        is_active: false,
        tags: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        created_by: None,
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let health = state
        .cluster_service
        .get_cluster_health_for_cluster(&temp_cluster)
        .await?;

    tracing::debug!("Connection test result: status={:?}", health.status);
    Ok(Json(health))
}
