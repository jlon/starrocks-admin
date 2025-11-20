use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;
use validator::Validate;

use crate::AppState;
use crate::models::{CreateFunctionRequest, UpdateFunctionRequest, UpdateOrderRequest};
use crate::utils::{ApiError, ApiResult};

// GET /api/clusters/system-functions
pub async fn get_system_functions(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<impl IntoResponse> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    let functions = state
        .system_function_service
        .get_functions(cluster.id)
        .await?;
    Ok(Json(functions))
}

// POST /api/clusters/system-functions
pub async fn create_system_function(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Extension(user_id): Extension<i64>,
    Json(req): Json<CreateFunctionRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    if let Err(validation_errors) = req.validate() {
        return Err(ApiError::validation_error(format!("请求参数验证失败：{}", validation_errors)));
    }

    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    let function = state
        .system_function_service
        .create_function(cluster.id, req, user_id)
        .await?;
    Ok((StatusCode::CREATED, Json(function)))
}

// POST /api/clusters/system-functions/:function_id/execute
pub async fn execute_system_function(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(function_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    let result = state
        .system_function_service
        .execute_function(cluster.id, function_id)
        .await?;
    Ok(Json(result))
}

// PUT /api/clusters/system-functions/orders
pub async fn update_function_orders(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(req): Json<UpdateOrderRequest>,
) -> ApiResult<impl IntoResponse> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    state
        .system_function_service
        .update_orders(cluster.id, req)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// PUT /api/clusters/system-functions/:function_id/favorite
pub async fn toggle_function_favorite(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(function_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    let function = state
        .system_function_service
        .toggle_favorite(cluster.id, function_id)
        .await?;
    Ok(Json(function))
}

// DELETE /api/clusters/system-functions/:function_id
pub async fn delete_system_function(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(function_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    state
        .system_function_service
        .delete_function(cluster.id, function_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// PUT /api/clusters/system-functions/:function_id
pub async fn update_function(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(function_id): Path<i64>,
    Json(req): Json<UpdateFunctionRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    if let Err(validation_errors) = req.validate() {
        return Err(ApiError::validation_error(format!("请求参数验证失败：{}", validation_errors)));
    }

    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };
    let function = state
        .system_function_service
        .update_function(cluster.id, function_id, req)
        .await?;
    Ok(Json(function))
}

// PUT /api/system-functions/:function_name/access-time
pub async fn update_system_function_access_time(
    State(state): State<Arc<AppState>>,
    Path(function_name): Path<String>,
) -> ApiResult<impl IntoResponse> {
    state
        .system_function_service
        .update_system_function_access_time(&function_name)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// DELETE /api/system-functions/category/:category_name
pub async fn delete_category(
    State(state): State<Arc<AppState>>,
    Path(category_name): Path<String>,
) -> ApiResult<impl IntoResponse> {
    state
        .system_function_service
        .delete_category(&category_name)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
