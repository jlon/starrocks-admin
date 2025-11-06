use axum::{Json, extract::State};
use std::sync::Arc;

use crate::AppState;
use crate::models::{PermissionResponse, PermissionTree};
use crate::utils::ApiResult;

// List all permissions
#[utoipa::path(
    get,
    path = "/api/permissions",
    responses(
        (status = 200, description = "List of permissions", body = Vec<PermissionResponse>)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Permissions"
)]
pub async fn list_permissions(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<PermissionResponse>>> {
    tracing::debug!("Listing all permissions");

    let permissions = state.permission_service.list_permissions().await?;

    tracing::debug!("Retrieved {} permissions", permissions.len());
    Ok(Json(permissions))
}

// List menu permissions
#[utoipa::path(
    get,
    path = "/api/permissions/menu",
    responses(
        (status = 200, description = "List of menu permissions", body = Vec<PermissionResponse>)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Permissions"
)]
pub async fn list_menu_permissions(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<PermissionResponse>>> {
    tracing::debug!("Listing menu permissions");

    let permissions = state.permission_service.list_menu_permissions().await?;

    tracing::debug!("Retrieved {} menu permissions", permissions.len());
    Ok(Json(permissions))
}

// List API permissions
#[utoipa::path(
    get,
    path = "/api/permissions/api",
    responses(
        (status = 200, description = "List of API permissions", body = Vec<PermissionResponse>)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Permissions"
)]
pub async fn list_api_permissions(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<PermissionResponse>>> {
    tracing::debug!("Listing API permissions");

    let permissions = state.permission_service.list_api_permissions().await?;

    tracing::debug!("Retrieved {} API permissions", permissions.len());
    Ok(Json(permissions))
}

// Get permissions as tree structure
#[utoipa::path(
    get,
    path = "/api/permissions/tree",
    responses(
        (status = 200, description = "Permission tree", body = Vec<PermissionTree>)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Permissions"
)]
pub async fn get_permission_tree(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<PermissionTree>>> {
    tracing::debug!("Getting permission tree");

    let tree = state.permission_service.get_permission_tree().await?;

    tracing::debug!("Retrieved permission tree with {} root nodes", tree.len());
    Ok(Json(tree))
}

// Get current user's permissions
#[utoipa::path(
    get,
    path = "/api/auth/permissions",
    responses(
        (status = 200, description = "Current user permissions", body = Vec<PermissionResponse>)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn get_current_user_permissions(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user_id): axum::extract::Extension<i64>,
) -> ApiResult<Json<Vec<PermissionResponse>>> {
    tracing::debug!("Getting permissions for user: ID={}", user_id);

    let permissions = state
        .permission_service
        .get_user_permissions(user_id)
        .await?;

    tracing::debug!("Retrieved {} permissions for user {}", permissions.len(), user_id);
    Ok(Json(permissions))
}
