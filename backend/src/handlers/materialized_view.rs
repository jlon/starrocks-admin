use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::AppState;
use crate::models::{
    AlterMaterializedViewRequest, CreateMaterializedViewRequest, MaterializedView,
    MaterializedViewDDL, RefreshMaterializedViewRequest,
};
use crate::services::{MaterializedViewService, MySQLClient};
use crate::utils::ApiResult;

#[derive(Debug, Deserialize)]
pub struct ListMVParams {
    pub database: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteMVParams {
    #[serde(default)]
    pub if_exists: bool,
}

#[derive(Debug, Deserialize)]
pub struct CancelRefreshParams {
    #[serde(default)]
    pub force: bool,
}

/// GET /api/clusters/materialized_views - List all materialized views
#[utoipa::path(
    get,
    path = "/api/clusters/materialized_views",
    params(
        ("database" = Option<String>, Query, description = "Database name filter"),
    ),
    responses(
        (status = 200, description = "List of materialized views", body = Vec<MaterializedView>),
        (status = 404, description = "No active cluster found")
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn list_materialized_views(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Query(params): Query<ListMVParams>,
) -> ApiResult<Json<Vec<MaterializedView>>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    let mvs = mv_service
        .list_materialized_views(params.database.as_deref())
        .await?;

    Ok(Json(mvs))
}

/// GET /api/clusters/materialized_views/{mv_name} - Get materialized view details
#[utoipa::path(
    get,
    path = "/api/clusters/materialized_views/{mv_name}",
    params(
        ("mv_name" = String, Path, description = "Materialized view name"),
    ),
    responses(
        (status = 200, description = "Materialized view details", body = MaterializedView),
        (status = 404, description = "Not found")
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn get_materialized_view(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(mv_name): Path<String>,
) -> ApiResult<Json<MaterializedView>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    let mv = mv_service.get_materialized_view(&mv_name).await?;
    Ok(Json(mv))
}

/// GET /api/clusters/{id}/materialized_views/{mv_name}/ddl - Get DDL statement
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/materialized_views/{mv_name}/ddl",
    params(
        ("id" = i64, Path, description = "Cluster ID"),
        ("mv_name" = String, Path, description = "Materialized view name"),
    ),
    responses(
        (status = 200, description = "DDL statement", body = MaterializedViewDDL),
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn get_materialized_view_ddl(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(mv_name): Path<String>,
) -> ApiResult<Json<MaterializedViewDDL>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    let ddl = mv_service.get_materialized_view_ddl(&mv_name).await?;
    Ok(Json(MaterializedViewDDL { mv_name: mv_name.clone(), ddl }))
}

/// POST /api/clusters/{id}/materialized_views - Create materialized view
#[utoipa::path(
    post,
    path = "/api/clusters/{id}/materialized_views",
    request_body = CreateMaterializedViewRequest,
    responses(
        (status = 201, description = "Created successfully"),
        (status = 400, description = "Invalid SQL"),
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn create_materialized_view(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Json(request): Json<CreateMaterializedViewRequest>,
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

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    mv_service.create_materialized_view(&request.sql).await?;

    Ok((StatusCode::CREATED, Json(json!({ "message": "Materialized view created successfully" }))))
}

/// DELETE /api/clusters/{id}/materialized_views/{mv_name} - Delete materialized view
#[utoipa::path(
    delete,
    path = "/api/clusters/{id}/materialized_views/{mv_name}",
    params(
        ("id" = i64, Path, description = "Cluster ID"),
        ("mv_name" = String, Path, description = "Materialized view name"),
        ("if_exists" = bool, Query, description = "Use IF EXISTS clause"),
    ),
    responses(
        (status = 200, description = "Deleted successfully"),
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn delete_materialized_view(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(mv_name): Path<String>,
    Query(params): Query<DeleteMVParams>,
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

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    mv_service
        .drop_materialized_view(&mv_name, params.if_exists)
        .await?;

    Ok((StatusCode::OK, Json(json!({ "message": "Materialized view deleted successfully" }))))
}

/// POST /api/clusters/{id}/materialized_views/{mv_name}/refresh - Refresh materialized view
#[utoipa::path(
    post,
    path = "/api/clusters/{id}/materialized_views/{mv_name}/refresh",
    request_body = RefreshMaterializedViewRequest,
    responses(
        (status = 200, description = "Refresh initiated"),
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn refresh_materialized_view(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(mv_name): Path<String>,
    Json(request): Json<RefreshMaterializedViewRequest>,
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

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    mv_service
        .refresh_materialized_view(
            &mv_name,
            request.partition_start.as_deref(),
            request.partition_end.as_deref(),
            request.force,
            &request.mode,
        )
        .await?;

    Ok((StatusCode::OK, Json(json!({ "message": "Refresh initiated" }))))
}

/// POST /api/clusters/{id}/materialized_views/{mv_name}/cancel - Cancel refresh
#[utoipa::path(
    post,
    path = "/api/clusters/{id}/materialized_views/{mv_name}/cancel",
    params(
        ("force" = bool, Query, description = "Force cancel"),
    ),
    responses(
        (status = 200, description = "Refresh cancelled"),
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn cancel_refresh_materialized_view(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(mv_name): Path<String>,
    Query(params): Query<CancelRefreshParams>,
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

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    mv_service
        .cancel_refresh_materialized_view(&mv_name, params.force)
        .await?;

    Ok((StatusCode::OK, Json(json!({ "message": "Refresh cancelled" }))))
}

/// PUT /api/clusters/{id}/materialized_views/{mv_name} - Alter materialized view
#[utoipa::path(
    put,
    path = "/api/clusters/{id}/materialized_views/{mv_name}",
    request_body = AlterMaterializedViewRequest,
    responses(
        (status = 200, description = "Altered successfully"),
    ),
    security(("bearer_auth" = [])),
    tag = "Materialized Views"
)]
pub async fn alter_materialized_view(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(mv_name): Path<String>,
    Json(request): Json<AlterMaterializedViewRequest>,
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

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let mv_service = MaterializedViewService::new(mysql_client);

    mv_service
        .alter_materialized_view(&mv_name, &request.alter_clause)
        .await?;

    Ok((StatusCode::OK, Json(json!({ "message": "Materialized view altered successfully" }))))
}
