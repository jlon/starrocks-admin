use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use std::sync::Arc;

use crate::{
    services::starrocks_client::StarRocksClient,
    utils::error::{ApiError, ApiResult},
};

/// Get query profile for a specific query
#[utoipa::path(
    get,
    path = "/api/clusters/queries/{query_id}/profile",
    params(
        ("query_id" = String, Path, description = "Query ID")
    ),
    responses(
        (status = 200, description = "Query profile", body = QueryProfile),
        (status = 404, description = "Query not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn get_query_profile(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(query_id): Path<String>,
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

    // Try to get profile from StarRocks
    let profile_result = get_profile_from_starrocks(&client, &query_id).await;

    match profile_result {
        Ok(profile) => Ok(Json(profile)),
        Err(_) => {
            // If profile not found, return a basic profile structure
            let basic_profile = QueryProfile {
                query_id: query_id.clone(),
                sql: "N/A".to_string(),
                profile_content: format!(
                    "Query profile for {} not found in StarRocks profile manager",
                    query_id
                ),
                execution_time_ms: 0,
                status: "Not Found".to_string(),
                fragments: vec![],
            };
            Ok(Json(basic_profile))
        },
    }
}

async fn get_profile_from_starrocks(
    client: &StarRocksClient,
    query_id: &str,
) -> ApiResult<QueryProfile> {
    // Note: SHOW PROC '/query_profile/{id}' path doesn't exist in StarRocks.
    // Query profile information is typically available via HTTP API /api/profile?query_id={id}
    // or stored in FE memory. For now, we return a message indicating the profile is not available via MySQL.
    let path = format!("/query_profile/{}", query_id);
    match client.show_proc_raw(&path).await {
        Ok(rows) if !rows.is_empty() => {
            let mut profile_content = String::new();
            for row in rows {
                if let serde_json::Value::Object(obj) = row {
                    for (key, value) in obj {
                        profile_content.push_str(&format!(
                            "{}: {}\n",
                            key,
                            value.as_str().unwrap_or_default()
                        ));
                    }
                    profile_content.push('\n');
                }
            }

            Ok(QueryProfile {
                query_id: query_id.to_string(),
                sql: "N/A".to_string(),
                profile_content,
                execution_time_ms: 0,
                status: "Completed".to_string(),
                fragments: Vec::new(),
            })
        },
        _ => {
            // Path doesn't exist or returned empty - return informative message
            Err(ApiError::not_found(format!(
                "Query profile '{}' not available via MySQL interface. Profile information may be available via HTTP API /api/profile?query_id={}",
                query_id, query_id
            )))
        },
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct QueryProfile {
    pub query_id: String,
    pub sql: String,
    pub profile_content: String,
    pub execution_time_ms: i64,
    pub status: String,
    pub fragments: Vec<QueryFragment>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct QueryFragment {
    pub fragment_id: String,
    pub instance_id: String,
    pub host: String,
    pub cpu_time_ns: i64,
    pub scan_rows: i64,
    pub scan_bytes: i64,
    pub memory_peak: i64,
}
