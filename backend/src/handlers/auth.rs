use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::models::{CreateUserRequest, LoginRequest, LoginResponse, UserResponse};
use crate::utils::ApiResult;

// Register a new user
#[utoipa::path(
    post,
    path = "/api/auth/register",
    request_body = CreateUserRequest,
    responses(
        (status = 200, description = "User registered successfully", body = UserResponse),
        (status = 400, description = "Bad request")
    ),
    tag = "Authentication"
)]
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateUserRequest>,
) -> ApiResult<Json<UserResponse>> {
    tracing::info!("User registration attempt for username: {}", req.username);
    tracing::debug!("Registration request: username={}, email={:?}", req.username, req.email);
    
    let user = state.auth_service.register(req).await?;
    
    tracing::info!("User registered successfully: {} (ID: {})", user.username, user.id);
    Ok(Json(user.into()))
}

// Login
#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials")
    ),
    tag = "Authentication"
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    tracing::info!("User login attempt for username: {}", req.username);
    tracing::debug!("Login request: username={}", req.username);
    
    let (user, token) = state.auth_service.login(req).await?;

    tracing::info!("User logged in successfully: {} (ID: {})", user.username, user.id);
    tracing::debug!("JWT token generated for user: {}", user.username);

    Ok(Json(LoginResponse {
        token,
        user: user.into(),
    }))
}

// Get current user info
#[utoipa::path(
    get,
    path = "/api/auth/me",
    responses(
        (status = 200, description = "Current user info", body = UserResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Authentication"
)]
pub async fn get_me(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user_id): axum::extract::Extension<i64>,
) -> ApiResult<Json<UserResponse>> {
    tracing::debug!("Getting user info for user_id: {}", user_id);
    
    let user = state.auth_service.get_user_by_id(user_id).await?;
    
    tracing::debug!("User info retrieved successfully: {} (ID: {})", user.username, user.id);
    Ok(Json(user.into()))
}

