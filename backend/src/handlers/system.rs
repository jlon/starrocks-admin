use axum::{extract::{Path, State}, Json};
use std::sync::Arc;

use crate::AppState;
use crate::models::RuntimeInfo;
use crate::services::StarRocksClient;
use crate::utils::ApiResult;

// Get runtime info for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/system/runtime_info",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Runtime information", body = RuntimeInfo),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "System"
)]
pub async fn get_runtime_info(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<RuntimeInfo>> {
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    let client = StarRocksClient::new(cluster);
    let runtime_info = client.get_runtime_info().await?;
    Ok(Json(runtime_info))
}

