use axum::{extract::{Path, State}, Json};
use std::sync::Arc;

use crate::AppState;
use crate::models::Frontend;
use crate::services::StarRocksClient;
use crate::utils::ApiResult;

// Get all frontends for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/frontends",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "List of frontend nodes", body = Vec<Frontend>),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Frontends"
)]
pub async fn list_frontends(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<Vec<Frontend>>> {
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    let client = StarRocksClient::new(cluster);
    let frontends = client.get_frontends().await?;
    Ok(Json(frontends))
}

