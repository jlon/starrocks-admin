use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::{
    models::starrocks::{UpdateVariableRequest, Variable},
    services::mysql_client::MySQLClient,
    utils::error::{ApiError, ApiResult},
};

#[derive(Debug, Deserialize)]
pub struct VariableQueryParams {
    #[serde(default = "default_type")]
    pub r#type: String, // "global" or "session"
    pub filter: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ConfigEntry {
    pub name: String,
    pub value: String,
}

fn default_type() -> String {
    "global".to_string()
}

/// Get system variables
#[utoipa::path(
    get,
    path = "/api/clusters/variables",
    params(
        ("type" = Option<String>, Query, description = "Variable type: global or session"),
        ("filter" = Option<String>, Query, description = "Filter variable name")
    ),
    responses(
        (status = 200, description = "Variables list", body = Vec<Variable>),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn get_variables(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Query(params): Query<VariableQueryParams>,
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

    // Get MySQL client from pool
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // Build SQL query
    let sql = match params.r#type.as_str() {
        "session" => "SHOW SESSION VARIABLES",
        _ => "SHOW GLOBAL VARIABLES",
    };

    let sql_with_filter = if let Some(filter) = params.filter {
        format!("{} LIKE '%{}%'", sql, filter)
    } else {
        sql.to_string()
    };

    // Execute query
    let (_, rows) = mysql_client.query_raw(&sql_with_filter).await?;

    // Parse results
    let variables: Vec<Variable> = rows
        .into_iter()
        .map(|row| Variable {
            name: row.first().cloned().unwrap_or_default(),
            value: row.get(1).cloned().unwrap_or_default(),
        })
        .collect();

    Ok(Json(variables))
}

/// Get FE configure info (SHOW FRONTEND CONFIG) and return as JSON
#[utoipa::path(
    get,
    path = "/api/clusters/configs",
    responses(
        (status = 200, description = "Frontend configure list", body = Vec<ConfigEntry>),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn get_configure_info(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<impl IntoResponse> {
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

    // Prefer SHOW FRONTEND CONFIG; fallback to SHOW CONFIG for compatibility
    let mut configs: Vec<ConfigEntry> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // Try admin + non-admin variants to maximize compatibility across versions/parsers
    for stmt in [
        "ADMIN SHOW FRONTEND CONFIG",
        "SHOW FRONTEND CONFIG",
        "ADMIN SHOW CONFIG",
        "SHOW CONFIG",
    ] {
        match mysql_client.query_raw(stmt).await {
            Ok((columns, rows)) => {
                // Log column names for debugging
                tracing::info!("Config query '{}' returned columns: {:?}", stmt, columns);
                
                // Find column indices for name and value
                let name_idx = columns.iter().position(|c| {
                    let lc = c.to_lowercase();
                    lc == "key" || lc == "name" || lc == "config_name" || lc == "configname"
                });
                let value_idx = columns.iter().position(|c| {
                    let lc = c.to_lowercase();
                    lc == "value" || lc == "config_value" || lc == "configvalue"
                });
                
                tracing::info!("name_idx: {:?}, value_idx: {:?}", name_idx, value_idx);
                
                if let Some(n_idx) = name_idx {
                    configs = rows
                        .into_iter()
                        .filter_map(|row| {
                            let name = row.get(n_idx).cloned().unwrap_or_default();
                            let value = value_idx
                                .and_then(|v_idx| row.get(v_idx).cloned())
                                .unwrap_or_default();
                            if name.is_empty() {
                                None
                            } else {
                                Some(ConfigEntry { name, value })
                            }
                        })
                        .collect();
                    
                    if !configs.is_empty() {
                        break;
                    }
                }
            },
            Err(e) => {
                errors.push(format!("{}: {}", stmt, e));
            },
        }
    }

    if configs.is_empty() && !errors.is_empty() {
        return Err(ApiError::internal_error(format!(
            "Failed to fetch FE config: {}",
            errors.join("; ")
        )));
    }

    Ok(Json(configs))
}

/// Update a variable
#[utoipa::path(
    put,
    path = "/api/clusters/variables/{variable_name}",
    params(
        ("variable_name" = String, Path, description = "Variable name")
    ),
    request_body = UpdateVariableRequest,
    responses(
        (status = 200, description = "Variable updated successfully"),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn update_variable(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(variable_name): Path<String>,
    Json(request): Json<UpdateVariableRequest>,
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

    // Get MySQL client from pool
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // Validate scope
    let scope = match request.scope.to_uppercase().as_str() {
        "GLOBAL" => "GLOBAL",
        "SESSION" => "SESSION",
        _ => return Err(ApiError::invalid_data("Invalid scope. Must be GLOBAL or SESSION")),
    };

    // Build SET command
    let sql = format!("SET {} {} = {}", scope, variable_name, request.value);

    // Execute command
    mysql_client.execute(&sql).await?;

    Ok((StatusCode::OK, Json(json!({ "message": "Variable updated successfully" }))))
}
