use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::AppState;
use crate::models::{
    CreateRoleRequest, RoleResponse, RoleWithPermissions, UpdateRolePermissionsRequest,
    UpdateRoleRequest,
};
use crate::utils::ApiResult;

// List all roles
#[utoipa::path(
    get,
    path = "/api/roles",
    responses(
        (status = 200, description = "List of roles", body = Vec<RoleResponse>)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Roles"
)]
pub async fn list_roles(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<RoleResponse>>> {
    tracing::debug!("Listing all roles");

    let roles = state.role_service.list_roles().await?;

    tracing::debug!("Retrieved {} roles", roles.len());
    Ok(Json(roles))
}

// Get role by ID
#[utoipa::path(
    get,
    path = "/api/roles/{id}",
    responses(
        (status = 200, description = "Role details", body = RoleResponse),
        (status = 404, description = "Role not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Roles"
)]
pub async fn get_role(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<RoleResponse>> {
    tracing::debug!("Getting role: ID={}", id);

    let role = state.role_service.get_role(id).await?;

    tracing::debug!("Retrieved role: {} (ID: {})", role.name, role.id);
    Ok(Json(role))
}

// Get role with permissions
#[utoipa::path(
    get,
    path = "/api/roles/{id}/permissions",
    responses(
        (status = 200, description = "Role with permissions", body = RoleWithPermissions),
        (status = 404, description = "Role not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Roles"
)]
pub async fn get_role_with_permissions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<RoleWithPermissions>> {
    tracing::debug!("Getting role with permissions: ID={}", id);

    let role_with_perms = state.role_service.get_role_with_permissions(id).await?;

    tracing::debug!(
        "Retrieved role {} with {} permissions",
        role_with_perms.role.name,
        role_with_perms.permissions.len()
    );
    Ok(Json(role_with_perms))
}

// Create a new role
#[utoipa::path(
    post,
    path = "/api/roles",
    request_body = CreateRoleRequest,
    responses(
        (status = 200, description = "Role created successfully", body = RoleResponse),
        (status = 400, description = "Bad request")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Roles"
)]
pub async fn create_role(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRoleRequest>,
) -> ApiResult<Json<RoleResponse>> {
    tracing::info!("Role creation request: code={}, name={}", req.code, req.name);
    tracing::debug!("Role creation details: code={}, description={:?}", req.code, req.description);

    let role = state.role_service.create_role(req).await?;

    tracing::info!("Role created successfully: {} (ID: {})", role.name, role.id);
    Ok(Json(role))
}

// Update role
#[utoipa::path(
    put,
    path = "/api/roles/{id}",
    request_body = UpdateRoleRequest,
    responses(
        (status = 200, description = "Role updated successfully", body = RoleResponse),
        (status = 404, description = "Role not found"),
        (status = 400, description = "Bad request")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Roles"
)]
pub async fn update_role(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateRoleRequest>,
) -> ApiResult<Json<RoleResponse>> {
    tracing::info!("Role update request: ID={}", id);
    tracing::debug!("Role update details: name={:?}, description={:?}", req.name, req.description);

    let role = state.role_service.update_role(id, req).await?;

    tracing::info!("Role updated successfully: {} (ID: {})", role.name, role.id);
    Ok(Json(role))
}

// Delete role
#[utoipa::path(
    delete,
    path = "/api/roles/{id}",
    responses(
        (status = 200, description = "Role deleted successfully"),
        (status = 404, description = "Role not found"),
        (status = 400, description = "Cannot delete system role")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Roles"
)]
pub async fn delete_role(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<Json<()>> {
    tracing::info!("Role deletion request: ID={}", id);

    state.role_service.delete_role(id).await?;

    tracing::info!("Role deleted successfully: ID={}", id);
    Ok(Json(()))
}

// Update role permissions
#[utoipa::path(
    put,
    path = "/api/roles/{id}/permissions",
    request_body = UpdateRolePermissionsRequest,
    responses(
        (status = 200, description = "Role permissions updated successfully"),
        (status = 404, description = "Role not found"),
        (status = 400, description = "Bad request")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Roles"
)]
pub async fn update_role_permissions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateRolePermissionsRequest>,
) -> ApiResult<Json<()>> {
    tracing::info!(
        "Role permissions update request: ID={}, permission_count={}",
        id,
        req.permission_ids.len()
    );
    tracing::debug!("Permission IDs: {:?}", req.permission_ids);

    state
        .role_service
        .assign_permissions_to_role(id, req)
        .await?;

    tracing::info!("Role permissions updated successfully: ID={}", id);
    Ok(Json(()))
}
