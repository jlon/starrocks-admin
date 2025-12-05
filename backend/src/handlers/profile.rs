use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::models::{ProfileDetail, ProfileListItem};
use crate::services::MySQLClient;
use crate::services::profile_analyzer::{
    AnalysisContext, ClusterVariables, ProfileAnalysisResponse, analyze_profile_with_context,
};
use crate::utils::{ApiResult, error::ApiError};

/// Validate and sanitize query_id to prevent SQL injection
/// StarRocks query_id format: UUID like "12345678-1234-1234-1234-123456789abc"
/// 
/// Returns the sanitized (trimmed) query_id as a String.
/// The sanitized version is what should be used for:
/// - SQL queries (security)
/// - API responses (consistency)
/// - Error messages (clarity)
fn sanitize_query_id(query_id: &str) -> Result<String, ApiError> {
    let id = query_id.trim();
    // Allow alphanumeric, hyphens, and underscores (UUID format)
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError::invalid_data("Invalid query_id format"));
    }
    // Return owned String to avoid lifetime issues and ensure consistency
    Ok(id.to_string())
}

// List all query profiles for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/profiles",
    responses(
        (status = 200, description = "List of query profiles", body = Vec<ProfileListItem>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Profiles"
)]
pub async fn list_profiles(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<Vec<ProfileListItem>>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    tracing::info!("Fetching profile list for cluster {}", cluster.id);

    // Get connection pool and execute SHOW PROFILELIST
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    let (columns, rows) = mysql_client.query_raw("SHOW PROFILELIST").await?;

    tracing::info!(
        "Profile list query returned {} rows with {} columns",
        rows.len(),
        columns.len()
    );

    // Convert rows to ProfileListItem, filtering out Aborted queries
    let profiles: Vec<ProfileListItem> = rows
        .into_iter()
        .filter(|row| {
            // Filter out Aborted state (index 3 is State column)
            let state = row.get(3).map(|s| s.as_str()).unwrap_or("");
            !state.eq_ignore_ascii_case("aborted")
        })
        .map(|row| {
            // SHOW PROFILELIST returns: QueryId, StartTime, Time, State, Statement
            ProfileListItem {
                query_id: row.first().cloned().unwrap_or_default(),
                start_time: row.get(1).cloned().unwrap_or_default(),
                time: row.get(2).cloned().unwrap_or_default(),
                state: row.get(3).cloned().unwrap_or_default(),
                statement: row.get(4).cloned().unwrap_or_default(),
            }
        })
        .collect();

    tracing::info!("Successfully converted {} profiles (Aborted filtered)", profiles.len());
    Ok(Json(profiles))
}

// Get detailed profile for a specific query
#[utoipa::path(
    get,
    path = "/api/clusters/profiles/{query_id}",
    params(
        ("query_id" = String, Path, description = "Query ID")
    ),
    responses(
        (status = 200, description = "Query profile detail", body = ProfileDetail),
        (status = 404, description = "No active cluster found or profile not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Profiles"
)]
pub async fn get_profile(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(query_id): Path<String>,
) -> ApiResult<Json<ProfileDetail>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    // Sanitize query_id to prevent SQL injection
    // Note: This trims whitespace and validates format. The sanitized version
    // is used consistently for SQL queries, responses, and error messages.
    let safe_query_id = sanitize_query_id(&query_id)?;
    
    // Log original vs sanitized if they differ (for debugging)
    if query_id.trim() != query_id {
        tracing::debug!(
            "Query ID sanitized: '{}' -> '{}'",
            query_id,
            safe_query_id
        );
    }
    
    tracing::info!("Fetching profile detail for query {} in cluster {}", safe_query_id, cluster.id);

    // Get connection pool and execute SELECT get_query_profile()
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    let sql = format!("SELECT get_query_profile('{}')", safe_query_id);
    let (_, rows) = mysql_client.query_raw(&sql).await?;

    // Extract profile content from result
    let profile_content = rows
        .first()
        .and_then(|row| row.first())
        .cloned()
        .unwrap_or_default();

    if profile_content.trim().is_empty() {
        return Err(ApiError::not_found(format!(
            "Profile not found for query: {}",
            safe_query_id
        )));
    }

    tracing::info!("Profile content length: {} bytes", profile_content.len());

    // Return sanitized query_id in response for consistency
    // This ensures the API contract is clear: responses use the sanitized (trimmed) version
    Ok(Json(ProfileDetail {
        query_id: safe_query_id,
        profile_content,
    }))
}

/// Analyze a query profile and return structured visualization data
#[utoipa::path(
    get,
    path = "/api/clusters/profiles/{query_id}/analyze",
    params(
        ("query_id" = String, Path, description = "Query ID to analyze")
    ),
    responses(
        (status = 200, description = "Profile analysis result with execution tree"),
        (status = 404, description = "No active cluster found or profile not found"),
        (status = 500, description = "Profile parsing failed")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Profiles"
)]
pub async fn analyze_profile_handler(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(query_id): Path<String>,
) -> ApiResult<Json<ProfileAnalysisResponse>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    // Sanitize query_id to prevent SQL injection
    // Note: This trims whitespace and validates format. The sanitized version
    // is used consistently for SQL queries, responses, and error messages.
    let safe_query_id = sanitize_query_id(&query_id)?;
    
    // Log original vs sanitized if they differ (for debugging)
    if query_id.trim() != query_id {
        tracing::debug!(
            "Query ID sanitized: '{}' -> '{}'",
            query_id,
            safe_query_id
        );
    }
    
    tracing::info!("Analyzing profile for query {} in cluster {}", safe_query_id, cluster.id);

    // Fetch profile content from StarRocks database (NOT from test files)
    // This ensures each query_id gets its actual profile data
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let sql = format!("SELECT get_query_profile('{}')", safe_query_id);
    let (_, rows) = mysql_client.query_raw(&sql).await?;

    // Extract profile content from database result
    let profile_content = rows
        .first()
        .and_then(|row| row.first())
        .cloned()
        .unwrap_or_default();

    if profile_content.trim().is_empty() {
        return Err(ApiError::not_found(format!(
            "Profile not found for query: {}",
            safe_query_id
        )));
    }

    tracing::info!(
        "Profile content length: {} bytes for query {}",
        profile_content.len(),
        safe_query_id
    );

    // Fetch live cluster session variables for smart parameter recommendations
    // Graceful degradation: if fetching fails, analysis continues without variables
    let cluster_variables = fetch_cluster_variables(&mysql_client).await;

    // Build analysis context with cluster variables
    let context = AnalysisContext { cluster_variables };

    // Parse the profile and return analysis with cluster context
    analyze_profile_with_context(&profile_content, &context)
        .map(Json)
        .map_err(|e| ApiError::internal_error(format!("Analysis failed: {}", e)))
}

/// Parameters we query from cluster for smart recommendations
/// These are used to provide context-aware parameter suggestions
const CLUSTER_VARIABLE_NAMES: &[&str] = &[
    "query_mem_limit",
    "query_timeout",
    "enable_spill",
    "pipeline_dop",
    "parallel_fragment_exec_instance_num",
    "io_tasks_per_scan_operator",
    "enable_global_runtime_filter",
    "runtime_join_filter_push_down_limit",
    "enable_scan_datacache",
    "enable_populate_datacache",
    "enable_query_cache",
    "pipeline_profile_level",
];

/// Fetch relevant session variables from the cluster
/// 
/// Returns `None` if query fails (graceful degradation).
/// This allows analysis to continue even if variable fetching fails,
/// though parameter recommendations may be less accurate.
async fn fetch_cluster_variables(mysql_client: &MySQLClient) -> Option<ClusterVariables> {
    // Build SQL query with parameterized variable names
    // Note: Variable names are constants, so SQL injection is not a concern here
    let sql = format!(
        "SHOW VARIABLES WHERE Variable_name IN ({})",
        CLUSTER_VARIABLE_NAMES
            .iter()
            .map(|name| format!("'{}'", name))
            .collect::<Vec<_>>()
            .join(",")
    );

    match mysql_client.query_raw(&sql).await {
        Ok((_, rows)) => {
            let mut variables = ClusterVariables::new();
            for row in rows {
                // SHOW VARIABLES returns: Variable_name, Value
                if row.len() >= 2 {
                    let var_name = row[0].clone();
                    let var_value = row[1].clone();
                    variables.insert(var_name, var_value);
                }
            }
            tracing::debug!(
                "Fetched {} cluster variables for smart recommendations",
                variables.len()
            );
            Some(variables)
        }
        Err(e) => {
            tracing::warn!(
                "Failed to fetch cluster variables: {}, analysis will continue without them",
                e
            );
            None
        }
    }
}
