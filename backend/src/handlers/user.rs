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
) -> ApiResult<Json<Vec<UserWithRolesResponse>>> {
    tracing::debug!("Listing users");
    let users = state.user_service.list_users().await?;
    tracing::debug!("Retrieved {} users", users.len());
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
) -> ApiResult<Json<UserWithRolesResponse>> {
    tracing::debug!("Fetching user_id={}", user_id);
    let user = state.user_service.get_user(user_id).await?;
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
    Json(payload): Json<AdminCreateUserRequest>,
) -> ApiResult<Json<UserWithRolesResponse>> {
    tracing::info!("Creating user: {}", payload.username);
    let user = state.user_service.create_user(payload).await?;
    tracing::info!("Created user: {} (ID: {})", user.user.username, user.user.id);
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
    Json(payload): Json<AdminUpdateUserRequest>,
) -> ApiResult<Json<UserWithRolesResponse>> {
    tracing::info!("Updating user_id={}", user_id);
    let user = state.user_service.update_user(user_id, payload).await?;
    tracing::info!("Updated user: {} (ID: {})", user.user.username, user.user.id);
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
) -> ApiResult<Json<()>> {
    tracing::info!("Deleting user_id={}", user_id);
    state.user_service.delete_user(user_id).await?;
    tracing::info!("Deleted user_id={}", user_id);
    Ok(Json(()))
}
