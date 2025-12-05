use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::models::{ProfileDetail, ProfileListItem};
use crate::services::MySQLClient;
use crate::services::profile_analyzer::{analyze_profile, ProfileAnalysisResponse};
use crate::utils::{ApiResult, error::ApiError};

/// Validate and sanitize query_id to prevent SQL injection
/// StarRocks query_id format: UUID like "12345678-1234-1234-1234-123456789abc"
fn sanitize_query_id(query_id: &str) -> Result<&str, ApiError> {
    let id = query_id.trim();
    // Allow alphanumeric, hyphens, and underscores (UUID format)
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(ApiError::invalid_data("Invalid query_id format"));
    }
    Ok(id)
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

    // Convert rows to ProfileListItem
    let profiles: Vec<ProfileListItem> = rows
        .into_iter()
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

    tracing::info!("Successfully converted {} profiles", profiles.len());
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
    let safe_query_id = sanitize_query_id(&query_id)?;
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
        return Err(ApiError::not_found(format!("Profile not found for query: {}", safe_query_id)));
    }

    tracing::info!("Profile content length: {} bytes", profile_content.len());

    Ok(Json(ProfileDetail { query_id: safe_query_id.to_string(), profile_content }))
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
        state.cluster_service.get_active_cluster_by_org(org_ctx.organization_id).await?
    };

    // Sanitize query_id to prevent SQL injection
    let safe_query_id = sanitize_query_id(&query_id)?;
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
        return Err(ApiError::not_found(format!("Profile not found for query: {}", safe_query_id)));
    }

    tracing::info!("Profile content length: {} bytes for query {}", profile_content.len(), safe_query_id);

    // Parse the profile and return analysis
    // Note: analyze_profile() only accepts string input, never reads from files
    analyze_profile(&profile_content)
        .map(Json)
        .map_err(|e| ApiError::internal_error(format!("Analysis failed: {}", e)))
}
