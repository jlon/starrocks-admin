use std::sync::Arc;

use axum::{Json, extract::Path, extract::State};

use crate::AppState;
use crate::models::{AdminCreateUserRequest, AdminUpdateUserRequest, UserWithRolesResponse};
use crate::utils::ApiResult;

/// List users with their roles
#[utoipa::path(
    get,
    path = "/api/users",
    responses(
        (status = 200, description = "List users with roles", body = Vec<UserWithRolesResponse>)
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<Vec<UserWithRolesResponse>>> {
    tracing::debug!(
        "Listing users for user {} (org: {:?}, super_admin: {})",
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    let users = state
        .user_service
        .list_users(org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;
    tracing::debug!("Retrieved {} users for user {}", users.len(), org_ctx.user_id);
    Ok(Json(users))
}

/// Get single user with roles
#[utoipa::path(
    get,
    path = "/api/users/{id}",
    responses(
        (status = 200, description = "User detail", body = UserWithRolesResponse),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<i64>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<UserWithRolesResponse>> {
    tracing::debug!(
        "Fetching user_id={} for user {} (org: {:?}, super_admin: {})",
        user_id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    let user = state
        .user_service
        .get_user(user_id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;
    tracing::debug!("Retrieved user {} for user {}", user.user.username, org_ctx.user_id);
    Ok(Json(user))
}

/// Create user with optional role assignments
#[utoipa::path(
    post,
    path = "/api/users",
    request_body = AdminCreateUserRequest,
    responses(
        (status = 200, description = "User created", body = UserWithRolesResponse),
        (status = 400, description = "Validation error"),
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(payload): Json<AdminCreateUserRequest>,
) -> ApiResult<Json<UserWithRolesResponse>> {
    if !org_ctx.is_super_admin && payload.organization_id.is_some() {
        return Err(crate::utils::ApiError::forbidden(
            "Organization administrators cannot override organization assignment",
        ));
    }

    tracing::info!(
        "Creating user: {} by user {} (org: {:?}, super_admin: {})",
        payload.username,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    let user = state
        .user_service
        .create_user(payload, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;
    tracing::info!(
        "Created user: {} (ID: {}) by user {}",
        user.user.username,
        user.user.id,
        org_ctx.user_id
    );
    Ok(Json(user))
}

/// Update user and role assignments
#[utoipa::path(
    put,
    path = "/api/users/{id}",
    request_body = AdminUpdateUserRequest,
    responses(
        (status = 200, description = "User updated", body = UserWithRolesResponse),
        (status = 404, description = "User not found"),
        (status = 400, description = "Validation error"),
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn update_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<i64>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(payload): Json<AdminUpdateUserRequest>,
) -> ApiResult<Json<UserWithRolesResponse>> {
    // Get existing user to check organization_id
    let existing = state
        .user_service
        .get_user(user_id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;

    // Only check if organization_id is being changed (not just present)
    if !org_ctx.is_super_admin
        && payload.organization_id.is_some()
        && payload.organization_id != existing.user.organization_id
    {
        return Err(crate::utils::ApiError::forbidden(
            "Organization administrators cannot reassign user organization",
        ));
    }

    tracing::info!(
        "Updating user_id={} by user {} (org: {:?}, super_admin: {})",
        user_id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    let user = state
        .user_service
        .update_user(user_id, payload, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;
    tracing::info!(
        "Updated user: {} (ID: {}) by user {}",
        user.user.username,
        user.user.id,
        org_ctx.user_id
    );
    Ok(Json(user))
}

/// Delete user and detach roles
#[utoipa::path(
    delete,
    path = "/api/users/{id}",
    responses(
        (status = 200, description = "User deleted"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "Users"
)]
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<i64>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<()>> {
    tracing::info!(
        "Deleting user_id={} by user {} (org: {:?}, super_admin: {})",
        user_id,
        org_ctx.user_id,
        org_ctx.organization_id,
        org_ctx.is_super_admin
    );
    state
        .user_service
        .delete_user(user_id, org_ctx.organization_id, org_ctx.is_super_admin)
        .await?;
    tracing::info!("Deleted user_id={} by user {}", user_id, org_ctx.user_id);
    Ok(Json(()))
}
