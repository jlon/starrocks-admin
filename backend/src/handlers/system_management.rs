use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{services::starrocks_client::StarRocksClient, utils::error::ApiResult};

#[derive(Debug, Deserialize)]
pub struct SystemQueryParams {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub filter: Option<String>,
    pub path: Option<String>, // New: support nested paths
}

/// Get system information list (all 25 system functions)
#[utoipa::path(
    get,
    path = "/api/clusters/system",
    params(
        ("limit" = Option<i32>, Query, description = "Limit results"),
        ("offset" = Option<i32>, Query, description = "Offset results"),
        ("filter" = Option<String>, Query, description = "Filter by name")
    ),
    responses(
        (status = 200, description = "System functions list", body = Vec<SystemFunction>),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn get_system_functions(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Query(params): Query<SystemQueryParams>,
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

    // Create StarRocks client (reserved for future extensions)
    let client = StarRocksClient::new(cluster, state.mysql_pool_manager.clone());

    // Get all system functions using SHOW PROC (currently static list)
    let functions = get_all_system_functions(&client, &params).await?;

    Ok(Json(functions))
}

/// Get detailed information for a specific system function
#[utoipa::path(
    get,
    path = "/api/clusters/system/{function_name}",
    params(
        ("function_name" = String, Path, description = "System function name"),
        ("path" = Option<String>, Query, description = "Nested path for hierarchical navigation")
    ),
    responses(
        (status = 200, description = "System function details", body = SystemFunctionDetail),
        (status = 404, description = "Function not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn get_system_function_detail(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(function_name): Path<String>,
    Query(params): Query<SystemQueryParams>,
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

    // Create StarRocks client
    let client = StarRocksClient::new(cluster, state.mysql_pool_manager.clone());

    // Build complete PROC path
    let proc_path = if let Some(nested_path) = params.path {
        format!("/{}/{}", function_name, nested_path)
    } else {
        format!("/{}", function_name)
    };

    // Get function details using HTTP REST API
    let detail = get_function_details(&client, &proc_path).await?;

    Ok(Json(detail))
}

async fn get_all_system_functions(
    _client: &StarRocksClient,
    params: &SystemQueryParams,
) -> ApiResult<Vec<SystemFunction>> {
    // Define all 25 system functions based on the screenshot
    let mut functions = vec![
        SystemFunction {
            name: "brokers".to_string(),
            description: "Broker nodes management".to_string(),
            category: "Storage".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "frontends".to_string(),
            description: "Frontend nodes management".to_string(),
            category: "Compute".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "routine_loads".to_string(),
            description: "Routine load jobs".to_string(),
            category: "Data Loading".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "catalog".to_string(),
            description: "Catalog management".to_string(),
            category: "Metadata".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "colocation_group".to_string(),
            description: "Colocation group management".to_string(),
            category: "Storage".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "cluster_balance".to_string(),
            description: "Cluster balance status".to_string(),
            category: "Balance".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "load_error_hub".to_string(),
            description: "Load error information".to_string(),
            category: "Data Loading".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "meta_recovery".to_string(),
            description: "Metadata recovery status".to_string(),
            category: "Metadata".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "global_current_queries".to_string(),
            description: "Global current queries".to_string(),
            category: "Query".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "tasks".to_string(),
            description: "System tasks".to_string(),
            category: "System".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "compute_nodes".to_string(),
            description: "Compute nodes management".to_string(),
            category: "Compute".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "statistic".to_string(),
            description: "Statistics information".to_string(),
            category: "Statistics".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "jobs".to_string(),
            description: "Background jobs".to_string(),
            category: "System".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "warehouses".to_string(),
            description: "Warehouse management".to_string(),
            category: "Compute".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "resources".to_string(),
            description: "Resource management".to_string(),
            category: "Resource".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "transactions".to_string(),
            description: "Transaction management".to_string(),
            category: "Transaction".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "backends".to_string(),
            description: "Backend nodes management".to_string(),
            category: "Storage".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "current_queries".to_string(),
            description: "Current running queries".to_string(),
            category: "Query".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "stream_loads".to_string(),
            description: "Stream load jobs".to_string(),
            category: "Data Loading".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "replications".to_string(),
            description: "Replication status".to_string(),
            category: "Replication".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "dbs".to_string(),
            description: "Database management".to_string(),
            category: "Metadata".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "current_backend_instances".to_string(),
            description: "Current backend instances".to_string(),
            category: "Storage".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "historical_nodes".to_string(),
            description: "Historical nodes".to_string(),
            category: "Storage".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
        SystemFunction {
            name: "compactions".to_string(),
            description: "Compaction tasks".to_string(),
            category: "Storage".to_string(),
            status: "Active".to_string(),
            last_updated: chrono::Utc::now(),
        },
    ];

    // Apply filtering
    if let Some(filter) = &params.filter {
        functions.retain(|f| f.name.contains(filter));
    }

    // Apply pagination
    let offset = params.offset.unwrap_or(0) as usize;
    let limit = params.limit.unwrap_or(25) as usize;

    let start = offset.min(functions.len());
    let end = (offset + limit).min(functions.len());

    Ok(functions[start..end].to_vec())
}

async fn get_function_details(
    client: &StarRocksClient,
    proc_path: &str,
) -> ApiResult<SystemFunctionDetail> {
    let mut detail_data = Vec::new();
    let rows = client.show_proc_raw(proc_path).await?;
    for value in rows {
        if let serde_json::Value::Object(obj) = value {
            let mut row_data = std::collections::HashMap::new();
            for (key, val) in obj {
                row_data.insert(key.clone(), val.as_str().unwrap_or("").to_string());
            }
            detail_data.push(row_data);
        }
    }

    // Extract function name from proc_path (e.g., "/brokers" -> "brokers")
    let function_name = proc_path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("unknown");

    Ok(SystemFunctionDetail {
        function_name: function_name.to_string(),
        description: get_function_description(function_name),
        data: detail_data.clone(),
        total_count: detail_data.len(),
        last_updated: chrono::Utc::now(),
    })
}

fn get_function_description(function_name: &str) -> String {
    match function_name {
        "brokers" => "Broker nodes for data loading and backup".to_string(),
        "frontends" => "Frontend nodes for query processing".to_string(),
        "backends" => "Backend nodes for data storage".to_string(),
        "current_queries" => "Currently running queries".to_string(),
        "dbs" => "Database information".to_string(),
        "routine_loads" => "Routine load jobs for continuous data loading".to_string(),
        "stream_loads" => "Stream load jobs for real-time data loading".to_string(),
        "transactions" => "Active transactions".to_string(),
        "jobs" => "Background system jobs".to_string(),
        "tasks" => "System tasks and their status".to_string(),
        "warehouses" => "Compute warehouses for resource isolation".to_string(),
        "resources" => "Resource pools and allocation".to_string(),
        "statistic" => "Table and column statistics".to_string(),
        "cluster_balance" => "Cluster data balance status".to_string(),
        "load_error_hub" => "Data loading error information".to_string(),
        "meta_recovery" => "Metadata recovery status".to_string(),
        "global_current_queries" => "Global query execution status".to_string(),
        "compute_nodes" => "Compute nodes for query execution".to_string(),
        "replications" => "Data replication status".to_string(),
        "current_backend_instances" => "Current backend instances".to_string(),
        "historical_nodes" => "Historical storage nodes".to_string(),
        "compactions" => "Data compaction tasks".to_string(),
        "colocation_group" => "Colocation groups for data locality".to_string(),
        "catalog" => "External catalogs".to_string(),
        _ => "System function".to_string(),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SystemFunction {
    pub name: String,
    pub description: String,
    pub category: String,
    pub status: String,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SystemFunctionDetail {
    pub function_name: String,
    pub description: String,
    pub data: Vec<std::collections::HashMap<String, String>>,
    pub total_count: usize,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}
