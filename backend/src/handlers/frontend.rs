use axum::{Json, extract::State};
use std::sync::Arc;

use crate::AppState;
use crate::models::Frontend;
use crate::services::StarRocksClient;
use crate::utils::ApiResult;

// Get all frontends for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/frontends",
    responses(
        (status = 200, description = "List of frontend nodes", body = Vec<Frontend>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Frontends"
)]
pub async fn list_frontends(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<Vec<Frontend>>> {
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
    let frontends = client.get_frontends().await?;
    Ok(Json(frontends))
}
