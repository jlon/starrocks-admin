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
    let client = StarRocksClient::new(cluster);

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
    // Try to get profile using HTTP REST API
    let url = format!("{}/api/show_proc?path=/query_profile/{}", client.get_base_url(), query_id);

    let response = client
        .http_client
        .get(&url)
        .basic_auth(&client.cluster.username, Some(&client.cluster.password_encrypted))
        .send()
        .await
        .map_err(|e| ApiError::cluster_connection_failed(format!("Request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(ApiError::cluster_connection_failed(format!(
            "HTTP status: {}",
            response.status()
        )));
    }

    let data: serde_json::Value = response.json().await.map_err(|e| {
        ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
    })?;

    // Parse profile data from JSON response
    let mut profile_content = String::new();
    let fragments = Vec::new();

    if let Some(array) = data.as_array() {
        for item in array {
            if let Some(obj) = item.as_object() {
                let mut line = String::new();
                for (key, value) in obj {
                    line.push_str(&format!("{}: {}\n", key, value.as_str().unwrap_or("")));
                }
                profile_content.push_str(&line);
                profile_content.push('\n');
            }
        }
    }

    Ok(QueryProfile {
        query_id: query_id.to_string(),
        sql: "N/A".to_string(), // SQL is not available in profile
        profile_content,
        execution_time_ms: 0, // Could be parsed from profile content
        status: "Completed".to_string(),
        fragments,
    })
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
