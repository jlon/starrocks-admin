use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::AppState;
use crate::models::Backend;
use crate::services::StarRocksClient;
use crate::utils::ApiResult;

// Get all backends for a cluster (BE nodes in shared-nothing, CN nodes in shared-data)
#[utoipa::path(
    get,
    path = "/api/clusters/backends",
    responses(
        (status = 200, description = "List of compute nodes (BE in shared-nothing, CN in shared-data)", body = Vec<Backend>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Backends"
)]
pub async fn list_backends(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<Vec<Backend>>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    let client = StarRocksClient::new(cluster, state.mysql_pool_manager.clone());
    let backends = client.get_backends().await?;
    Ok(Json(backends))
}

// Delete a backend/compute node (BE in shared-nothing, CN in shared-data)
#[utoipa::path(
    delete,
    path = "/api/clusters/backends/{host}/{port}",
    params(
        ("host" = String, Path, description = "Node host"),
        ("port" = String, Path, description = "Node heartbeat port")
    ),
    responses(
        (status = 200, description = "Node deleted successfully"),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Failed to delete node")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Backends"
)]
pub async fn delete_backend(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path((host, port)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    tracing::info!("Deleting backend {}:{} from cluster {}", host, port, cluster.id);

    let client = StarRocksClient::new(cluster, state.mysql_pool_manager.clone());
    client.drop_backend(&host, &port).await?;

    Ok(Json(serde_json::json!({
        "message": format!("Backend {}:{} deleted successfully", host, port)
    })))
}
