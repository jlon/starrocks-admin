use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

use crate::AppState;
use crate::models::{Query, QueryExecuteRequest, QueryExecuteResponse};
use crate::services::{MySQLClient, StarRocksClient};
use crate::utils::ApiResult;

// Get all running queries for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/{id}/queries",
    params(
        ("id" = i64, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "List of running queries", body = Vec<Query>),
        (status = 404, description = "Cluster not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn list_queries(
    State(state): State<Arc<AppState>>,
    Path(cluster_id): Path<i64>,
) -> ApiResult<Json<Vec<Query>>> {
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    let client = StarRocksClient::new(cluster);
    let queries = client.get_queries().await?;
    Ok(Json(queries))
}

// Kill a query
#[utoipa::path(
    delete,
    path = "/api/clusters/{cluster_id}/queries/{query_id}",
    params(
        ("cluster_id" = i64, Path, description = "Cluster ID"),
        ("query_id" = String, Path, description = "Query ID")
    ),
    responses(
        (status = 200, description = "Query killed successfully"),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn kill_query(
    State(state): State<Arc<crate::AppState>>,
    Path((cluster_id, query_id)): Path<(i64, String)>,
) -> ApiResult<impl IntoResponse> {
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // Execute KILL QUERY
    let sql = format!("KILL QUERY '{}'", query_id);
    mysql_client.execute(&sql).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "message": "Query killed successfully" })),
    ))
}

// Execute SQL query
#[utoipa::path(
    post,
    path = "/api/clusters/{cluster_id}/queries/execute",
    params(
        ("cluster_id" = i64, Path, description = "Cluster ID")
    ),
    request_body = QueryExecuteRequest,
    responses(
        (status = 200, description = "Query executed successfully", body = QueryExecuteResponse),
        (status = 400, description = "Invalid SQL or query error"),
        (status = 404, description = "Cluster not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn execute_sql(
    State(state): State<Arc<crate::AppState>>,
    Path(cluster_id): Path<i64>,
    Json(request): Json<QueryExecuteRequest>,
) -> ApiResult<Json<QueryExecuteResponse>> {
    let cluster = state.cluster_service.get_cluster(cluster_id).await?;
    
    // DEBUG: Print cluster info
    tracing::info!("🔍 Starting SQL execution - Cluster: ID={}, Host={}, Port={}, User={}", 
                   cluster.id, cluster.fe_host, cluster.fe_query_port, cluster.username);
    
    // Use pool manager to get cached pool (avoid intermittent failures from creating new pools)
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    let original_sql = &request.sql;
    let sql = apply_query_limit(original_sql, request.limit.unwrap_or(1000));
    tracing::info!("📝 SQL Query - Original: '{}'", original_sql);
    tracing::info!("📝 SQL Query - Modified: '{}'", sql);

    let start = Instant::now();

    // Execute query with detailed error handling
    tracing::info!("🚀 Executing SQL query...");
    let query_result = mysql_client.query_raw(&sql).await;
    
    let execution_time_ms = start.elapsed().as_millis();
    
    match query_result {
        Ok((columns, data_rows)) => {
            let row_count = data_rows.len();
            tracing::info!("✅ Query executed successfully - Rows: {}, Time: {}ms", row_count, execution_time_ms);
            
            // Log first few rows for debugging
            if row_count > 0 {
                tracing::info!("📊 Query result preview - First row: {:?}", 
                              data_rows[0].iter().take(3).collect::<Vec<_>>());
            }
            
            Ok(Json(QueryExecuteResponse {
                columns,
                rows: data_rows,
                row_count,
                execution_time_ms,
            }))
        }
        Err(e) => {
            tracing::error!("❌ Query execution failed - Error: {:?}, Time: {}ms", e, execution_time_ms);
            tracing::error!("❌ Failed SQL: '{}'", sql);
            Err(e)
        }
    }
}

fn apply_query_limit(sql: &str, limit: i32) -> String {
    let sql_upper = sql.trim().to_uppercase();
    tracing::debug!("apply_query_limit: input='{}', upper='{}'", sql, sql_upper);
    
    if sql_upper.contains("LIMIT") {
        tracing::debug!("apply_query_limit: contains LIMIT, returning original");
        return sql.to_string();
    }
    
    if sql_upper.starts_with("SELECT") {
        if sql_upper.contains("GET_QUERY_PROFILE") || 
           sql_upper.contains("SHOW_PROFILE") ||
           sql_upper.contains("EXPLAIN") {
            tracing::debug!("apply_query_limit: special query, returning original");
            return sql.to_string();
        }
        
        let result = format!("{} LIMIT {}", sql.trim().trim_end_matches(';'), limit);
        tracing::debug!("apply_query_limit: adding LIMIT, result='{}'", result);
        result
    } else {
        tracing::debug!("apply_query_limit: not SELECT, returning original");
        sql.to_string()
    }
}