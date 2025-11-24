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
pub async fn list_roles(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<Vec<RoleResponse>>> {
    tracing::debug!(
        "Listing roles for user {} (org: {:?}, super_admin: {})",
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );

    let roles = state
        .role_service
        .list_roles(org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    tracing::debug!("Retrieved {} roles for user {}", roles.len(), org_ctx.user_id);
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<RoleResponse>> {
    tracing::debug!(
        "Getting role: ID={} for user {} (org: {:?}, super_admin: {})",
        id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );

    let role = state
        .role_service
        .get_role(id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    tracing::debug!("Retrieved role: {} (ID: {}) for user {}", role.name, role.id, org_ctx.user_id);
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<RoleWithPermissions>> {
    tracing::debug!(
        "Getting role with permissions: ID={} for user {} (org: {:?}, super_admin: {})",
        id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );

    let role_with_perms = state
        .role_service
        .get_role_with_permissions(id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    tracing::debug!(
        "Retrieved role {} with {} permissions for user {}",
        role_with_perms.role.name,
        role_with_perms.permissions.len(),
        org_ctx.user_id
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(req): Json<CreateRoleRequest>,
) -> ApiResult<Json<RoleResponse>> {
    if !org_ctx.is_super_admin && req.organization_id.is_some() {
        return Err(crate::utils::ApiError::forbidden(
            "Organization administrators cannot override organization assignment",
        ));
    }

    tracing::info!(
        "Role creation request: code={}, name={} by user {} (org: {:?}, super_admin: {})",
        req.code,
        req.name,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    tracing::debug!("Role creation details: code={}, description={:?}", req.code, req.description);

    let role = state
        .role_service
        .create_role(req, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    tracing::info!(
        "Role created successfully: {} (ID: {}) by user {}",
        role.name,
        role.id,
        org_ctx.user_id
    );
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(req): Json<UpdateRoleRequest>,
) -> ApiResult<Json<RoleResponse>> {
    // Get existing role to check organization_id
    let existing = state
        .role_service
        .get_role(id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    // Only check if organization_id is being changed (not just present)
    if !org_ctx.is_super_admin
        && req.organization_id.is_some()
        && req.organization_id != existing.organization_id
    {
        return Err(crate::utils::ApiError::forbidden(
            "Organization administrators cannot reassign role organization",
        ));
    }

    tracing::info!(
        "Role update request: ID={} by user {} (org: {:?}, super_admin: {})",
        id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    tracing::debug!("Role update details: name={:?}, description={:?}", req.name, req.description);

    let role = state
        .role_service
        .update_role(id, req, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    tracing::info!(
        "Role updated successfully: {} (ID: {}) by user {}",
        role.name,
        role.id,
        org_ctx.user_id
    );
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<()>> {
    tracing::info!(
        "Role deletion request: ID={} by user {} (org: {:?}, super_admin: {})",
        id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );

    state
        .role_service
        .delete_role(id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    tracing::info!("Role deleted successfully: ID={} by user {}", id, org_ctx.user_id);
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
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(req): Json<UpdateRolePermissionsRequest>,
) -> ApiResult<Json<()>> {
    tracing::info!(
        "Role permissions update request: ID={}, permission_count={} by user {} (org: {:?}, super_admin: {})",
        id,
        req.permission_ids.len(),
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    tracing::debug!("Permission IDs: {:?}", req.permission_ids);

    state
        .role_service
        .assign_permissions_to_role(id, req, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    tracing::info!("Role permissions updated successfully: ID={} by user {}", id, org_ctx.user_id);
    Ok(Json(()))
}
