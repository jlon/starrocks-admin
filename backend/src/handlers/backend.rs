use axum::{extract::{Path, State}, Json};
use std::sync::Arc;

use crate::AppState;
use crate::models::Backend;
use crate::services::StarRocksClient;
use crate::utils::ApiResult;

// Get all backends for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/backends",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "List of backend nodes", body = Vec<Backend>),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Backends"
)]
pub async fn list_backends(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<Vec<Backend>>> {
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    let client = StarRocksClient::new(cluster);
    let backends = client.get_backends().await?;
    Ok(Json(backends))
}

// Delete a backend node
#[utoipa::path(
    delete,
    path = "/api/clusters/{id}/backends/{host}/{port}",
    params(
        ("id" = i64, Path, description = "Cluster ID"),
        ("host" = String, Path, description = "Backend host"),
        ("port" = String, Path, description = "Backend heartbeat port")
    ),
    responses(
        (status = 200, description = "Backend deleted successfully"),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Failed to delete backend")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Backends"
)]
pub async fn delete_backend(
    State(state): State<Arc<AppState>>,
    Path((cluster_id, host, port)): Path<(i64, String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    tracing::info!("Deleting backend {}:{} from cluster {}", host, port, cluster_id);
    
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    let client = StarRocksClient::new(cluster);
    client.drop_backend(&host, &port).await?;
    
    Ok(Json(serde_json::json!({
        "message": format!("Backend {}:{} deleted successfully", host, port)
    })))
}

