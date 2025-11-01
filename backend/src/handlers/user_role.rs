use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::AppState;
use crate::models::{AssignUserRoleRequest, RoleResponse};
use crate::utils::ApiResult;

// Get user's roles
#[utoipa::path(
    get,
    path = "/api/users/{id}/roles",
    responses(
        (status = 200, description = "User roles", body = Vec<RoleResponse>),
        (status = 404, description = "User not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Users"
)]
pub async fn get_user_roles(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<RoleResponse>>> {
    tracing::debug!("Getting roles for user: ID={}", id);

    let roles = state.user_role_service.get_user_roles(id).await?;

    tracing::debug!("Retrieved {} roles for user {}", roles.len(), id);
    Ok(Json(roles))
}

// Assign role to user
#[utoipa::path(
    post,
    path = "/api/users/{id}/roles",
    request_body = AssignUserRoleRequest,
    responses(
        (status = 200, description = "Role assigned successfully"),
        (status = 404, description = "User or role not found"),
        (status = 400, description = "User already has this role")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Users"
)]
pub async fn assign_role_to_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<AssignUserRoleRequest>,
) -> ApiResult<Json<()>> {
    tracing::info!("Assigning role to user: user_id={}, role_id={}", id, req.role_id);
    tracing::debug!("Role assignment request: user_id={}, role_id={}", id, req.role_id);

    state.user_role_service.assign_role_to_user(id, req).await?;

    tracing::info!("Role assigned successfully to user {}", id);
    Ok(Json(()))
}

// Remove role from user
#[utoipa::path(
    delete,
    path = "/api/users/{id}/roles/{role_id}",
    responses(
        (status = 200, description = "Role removed successfully"),
        (status = 404, description = "User role assignment not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Users"
)]
pub async fn remove_role_from_user(
    State(state): State<Arc<AppState>>,
    Path((id, role_id)): Path<(i64, i64)>,
) -> ApiResult<Json<()>> {
    tracing::info!("Removing role from user: user_id={}, role_id={}", id, role_id);
    tracing::debug!("Role removal request: user_id={}, role_id={}", id, role_id);

    state
        .user_role_service
        .remove_role_from_user(id, role_id)
        .await?;

    tracing::info!("Role removed successfully from user {}", id);
    Ok(Json(()))
}
