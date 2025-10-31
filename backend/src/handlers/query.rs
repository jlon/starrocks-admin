use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

use crate::AppState;
use crate::models::{CatalogWithDatabases, CatalogsWithDatabasesResponse, Query, QueryExecuteRequest, QueryExecuteResponse};
use crate::services::mysql_client::MySQLClient;
use crate::services::StarRocksClient;
use crate::utils::ApiResult;

// Get list of catalogs using MySQL client
#[utoipa::path(
    get,
    path = "/api/clusters/catalogs",
    responses(
        (status = 200, description = "List of catalogs", body = Vec<String>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn list_catalogs(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<String>>> {
    let cluster = state.cluster_service.get_active_cluster().await?;
    
    // Use MySQL client to execute SHOW CATALOGS
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    
    let (_, rows) = mysql_client.query_raw("SHOW CATALOGS", None, None).await?;
    
    let mut catalogs = Vec::new();
    for row in rows {
        if let Some(catalog_name) = row.first() {
            let name = catalog_name.trim().to_string();
            if !name.is_empty() {
                catalogs.push(name);
            }
        }
    }
    
    tracing::debug!("Found {} catalogs via MySQL client", catalogs.len());
    Ok(Json(catalogs))
}

// Get list of databases in a catalog using MySQL client
#[utoipa::path(
    get,
    path = "/api/clusters/databases",
    params(
        ("catalog" = Option<String>, Query, description = "Catalog name (optional)")
    ),
    responses(
        (status = 200, description = "List of databases", body = Vec<String>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn list_databases(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<Vec<String>>> {
    let cluster = state.cluster_service.get_active_cluster().await?;
    
    // Use MySQL client
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    
    // Get catalog parameter if provided
    if let Some(catalog_name) = params.get("catalog") {
        // Skip switching to default_catalog - it's already active by default
        if !catalog_name.is_empty() && catalog_name != "default_catalog" {
            // First switch to the catalog, then show databases
            // Try without backticks - StarRocks may not support them for catalog names
            let use_catalog_sql = format!("USE CATALOG {}", catalog_name);
            if let Err(e) = mysql_client.execute(&use_catalog_sql).await {
                tracing::warn!("Failed to switch to catalog {}: {}", catalog_name, e);
                // Continue anyway, might be using default catalog
            }
        }
    }
    
    // Execute SHOW DATABASES
    let (_, rows) = mysql_client.query_raw("SHOW DATABASES", None, None).await?;
    
    let mut databases = Vec::new();
    for row in rows {
        if let Some(db_name) = row.first() {
            let name = db_name.trim().to_string();
            // Skip system databases
            if !name.is_empty()
                && name != "information_schema" 
                && name != "_statistics_" {
                databases.push(name);
            }
        }
    }
    
    tracing::debug!("Found {} databases via MySQL client", databases.len());
    Ok(Json(databases))
}

// Get all catalogs with their databases using MySQL client (one-time response)
#[utoipa::path(
    get,
    path = "/api/clusters/catalogs-databases",
    responses(
        (status = 200, description = "All catalogs with their databases", body = CatalogsWithDatabasesResponse),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn list_catalogs_with_databases(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<CatalogsWithDatabasesResponse>> {
    let cluster = state.cluster_service.get_active_cluster().await?;
    
    // Use MySQL client
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    
    // Step 1: Get all catalogs
    let (_, catalog_rows) = mysql_client.query_raw("SHOW CATALOGS", None, None).await?;
    
    let mut catalogs = Vec::new();
    
    // Extract catalog names
    let mut catalog_names = Vec::new();
    for row in catalog_rows {
        if let Some(catalog_name) = row.first() {
            let name = catalog_name.trim().to_string();
            if !name.is_empty() {
                catalog_names.push(name);
            }
        }
    }
    
    tracing::debug!("Found {} catalogs, fetching databases for each...", catalog_names.len());
    
    // Step 2: For each catalog, switch to it and get databases
    for catalog_name in &catalog_names {
        // Skip switching to default_catalog - it's already active by default
        if catalog_name != "default_catalog" {
            // Switch to catalog (without backticks - StarRocks may not support them for catalog names)
            let use_catalog_sql = format!("USE CATALOG {}", catalog_name);
            if let Err(e) = mysql_client.execute(&use_catalog_sql).await {
                tracing::warn!("Failed to switch to catalog {}: {}", catalog_name, e);
                catalogs.push(CatalogWithDatabases {
                    catalog: catalog_name.clone(),
                    databases: Vec::new(),
                });
                continue;
            }
        }
        
        // Get databases for this catalog
        let (_, db_rows) = match mysql_client.query_raw("SHOW DATABASES", None, None).await {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("Failed to get databases for catalog {}: {}", catalog_name, e);
                catalogs.push(CatalogWithDatabases {
                    catalog: catalog_name.clone(),
                    databases: Vec::new(),
                });
                continue;
            }
        };
        
        let mut databases = Vec::new();
        for row in db_rows {
            if let Some(db_name) = row.first() {
                let name = db_name.trim().to_string();
                // Skip system databases
                if !name.is_empty()
                    && name != "information_schema" 
                    && name != "_statistics_" {
                    databases.push(name);
                }
            }
        }
        
        tracing::debug!("Catalog {} has {} databases", catalog_name, databases.len());
        
        catalogs.push(CatalogWithDatabases {
            catalog: catalog_name.clone(),
            databases,
        });
    }
    
    Ok(Json(CatalogsWithDatabasesResponse { catalogs }))
}

// Get all running queries for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/queries",
    responses(
        (status = 200, description = "List of running queries", body = Vec<Query>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn list_queries(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<Query>>> {
    let cluster = state.cluster_service.get_active_cluster().await?;
    
    tracing::info!("[list_queries] Fetching running queries for cluster: {} (ID: {})", cluster.name, cluster.id);
    
    // Try HTTP API first
    let client = StarRocksClient::new(cluster.clone());
    match client.get_queries().await {
        Ok(queries) if !queries.is_empty() => {
            tracing::info!("[list_queries] Successfully retrieved {} running queries via HTTP API", queries.len());
            return Ok(Json(queries));
        },
        Ok(queries) => {
            tracing::info!("[list_queries] HTTP API returned empty list ({} queries), will try MySQL client as fallback", queries.len());
        },
        Err(e) => {
            tracing::warn!("[list_queries] HTTP API failed to fetch queries: {}. Will try MySQL client as fallback", e);
        },
    }
    
    // Fallback: Try using MySQL client to query via SHOW PROC '/current_queries'
    // Note: StarRocks supports SHOW PROC via MySQL protocol in some versions
    tracing::info!("[list_queries] Attempting to fetch queries via MySQL client as fallback");
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    
    // Try SHOW PROC '/current_queries' via MySQL
    match mysql_client.query_raw("SHOW PROC '/current_queries'", None, None).await {
        Ok((columns, rows)) => {
            tracing::info!("[list_queries] MySQL SHOW PROC returned {} rows with {} columns: {:?}", 
                rows.len(), columns.len(), columns);
            
            if rows.is_empty() {
                tracing::info!("[list_queries] No running queries found via MySQL SHOW PROC");
                return Ok(Json(Vec::new()));
            }
            
            // Parse rows into Query objects
            // Actual columns from SHOW PROC '/current_queries':
            // StartTime, feIp, QueryId, ConnectionId, Database, User, ScanBytes, ScanRows, 
            // MemoryUsage, DiskSpillSize, CPUTime, ExecTime, ExecProgress, Warehouse, CustomQueryId, ResourceGroup
            let mut queries = Vec::new();
            
            // Find column indices (case-insensitive matching for robustness)
            let find_col = |name: &str| -> Option<usize> {
                columns.iter().position(|c| c.eq_ignore_ascii_case(name))
            };
            
            let query_id_idx = find_col("QueryId");
            let connection_id_idx = find_col("ConnectionId");
            let database_idx = find_col("Database");
            let user_idx = find_col("User");
            let scan_bytes_idx = find_col("ScanBytes");
            let scan_rows_idx = find_col("ScanRows");  // Changed from ProcessRows
            let cpu_time_idx = find_col("CPUTime");
            let exec_time_idx = find_col("ExecTime");
            // Sql column may not exist in PROC output, try ExecProgress or use empty string
            let sql_idx = find_col("Sql").or_else(|| find_col("ExecProgress")).or_else(|| find_col("Statement"));
            
            tracing::debug!("[list_queries] Column indices - QueryId: {:?}, ConnectionId: {:?}, Database: {:?}, User: {:?}, ScanRows: {:?}, Sql: {:?}", 
                query_id_idx, connection_id_idx, database_idx, user_idx, scan_rows_idx, sql_idx);
            tracing::debug!("[list_queries] All available columns: {:?}", columns);
            
            for (row_idx, row) in rows.iter().enumerate() {
                // Required columns for Query struct
                if let (Some(query_id_idx), Some(connection_id_idx), Some(database_idx), 
                        Some(user_idx), Some(scan_bytes_idx), Some(scan_rows_idx),
                        Some(cpu_time_idx), Some(exec_time_idx)) = 
                    (query_id_idx, connection_id_idx, database_idx, user_idx,
                     scan_bytes_idx, scan_rows_idx, cpu_time_idx, exec_time_idx) {
                    
                    // Sql is optional - use ExecProgress if Sql doesn't exist, or empty string
                    let sql_value = sql_idx.and_then(|idx| row.get(idx).cloned()).unwrap_or_else(|| {
                        // Try to get ExecProgress as fallback
                        find_col("ExecProgress").and_then(|idx| row.get(idx).cloned()).unwrap_or_default()
                    });
                    
                    let query = Query {
                        query_id: row.get(query_id_idx).cloned().unwrap_or_default(),
                        connection_id: row.get(connection_id_idx).cloned().unwrap_or_default(),
                        database: row.get(database_idx).cloned().unwrap_or_default(),
                        user: row.get(user_idx).cloned().unwrap_or_default(),
                        scan_bytes: row.get(scan_bytes_idx).cloned().unwrap_or_default(),
                        process_rows: row.get(scan_rows_idx).cloned().unwrap_or_default(), // Maps to ScanRows
                        cpu_time: row.get(cpu_time_idx).cloned().unwrap_or_default(),
                        exec_time: row.get(exec_time_idx).cloned().unwrap_or_default(),
                        sql: sql_value,
                    };
                    tracing::debug!("[list_queries] Parsed query {}: QueryId={}, User={}, Database={}, ExecTime={}", 
                        row_idx, query.query_id, query.user, query.database, query.exec_time);
                    queries.push(query);
                } else {
                    tracing::warn!("[list_queries] Missing required columns in row {}. Available columns: {:?}", row_idx, columns);
                    tracing::warn!("[list_queries] Found indices - QueryId: {:?}, ConnectionId: {:?}, Database: {:?}, User: {:?}, ScanRows: {:?}, Sql: {:?}", 
                        query_id_idx, connection_id_idx, database_idx, user_idx, scan_rows_idx, sql_idx);
                    // Continue processing other rows instead of breaking
                }
            }
            
            if !queries.is_empty() {
                tracing::info!("[list_queries] Successfully parsed {} queries from MySQL SHOW PROC", queries.len());
                return Ok(Json(queries));
            } else {
                tracing::warn!("[list_queries] MySQL SHOW PROC returned {} rows but failed to parse any queries", rows.len());
            }
        },
        Err(e) => {
            tracing::warn!("[list_queries] MySQL SHOW PROC '/current_queries' failed: {}", e);
        },
    }
    
    // If both methods failed or returned empty, return empty list
    tracing::info!("[list_queries] No running queries found (both HTTP API and MySQL methods tried)");
    Ok(Json(Vec::new()))
}

// Kill a query
#[utoipa::path(
    delete,
    path = "/api/clusters/queries/{query_id}",
    params(
        ("query_id" = String, Path, description = "Query ID")
    ),
    responses(
        (status = 200, description = "Query killed successfully"),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn kill_query(
    State(state): State<Arc<crate::AppState>>,
    Path(query_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let cluster = state.cluster_service.get_active_cluster().await?;

    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // Execute KILL QUERY
    let sql = format!("KILL QUERY '{}'", query_id);
    mysql_client.execute(&sql).await?;

    Ok((StatusCode::OK, Json(json!({ "message": "Query killed successfully" }))))
}

// Execute SQL query
// If database is provided, will execute USE database before the SQL query
#[utoipa::path(
    post,
    path = "/api/clusters/queries/execute",
    request_body = QueryExecuteRequest,
    responses(
        (status = 200, description = "Query executed successfully", body = QueryExecuteResponse),
        (status = 400, description = "Invalid SQL or query error"),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn execute_sql(
    State(state): State<Arc<crate::AppState>>,
    Json(request): Json<QueryExecuteRequest>,
) -> ApiResult<Json<QueryExecuteResponse>> {
    let cluster = state.cluster_service.get_active_cluster().await?;

    // Use pool manager to get cached pool (avoid intermittent failures from creating new pools)
    let pool: mysql_async::Pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // If catalog is specified, switch to it first
    if let Some(ref cat) = request.catalog {
        // Skip switching to default_catalog - it's already active by default
        if !cat.is_empty() && cat != "default_catalog" {
            let use_catalog_sql = format!("USE CATALOG `{}`", cat);
            tracing::debug!("Executing USE CATALOG: {}", use_catalog_sql);
            if let Err(e) = mysql_client.execute(&use_catalog_sql).await {
                tracing::warn!("Failed to execute USE CATALOG {}: {}", cat, e);
                // Continue execution anyway - catalog might already be active
            }
        }
    }

    // If database is specified, execute USE database
    if let Some(ref db) = request.database {
        if !db.is_empty() {
            let use_db_sql = format!("USE `{}`", db);
            tracing::debug!("Executing USE DATABASE: {}", use_db_sql);
            if let Err(e) = mysql_client.execute(&use_db_sql).await {
                tracing::warn!("Failed to execute USE database {}: {}", db, e);
                // Continue execution anyway - database might not exist
            }
        }
    }

    let original_sql = &request.sql;
    let sql = apply_query_limit(original_sql, request.limit.unwrap_or(1000));

    let start = Instant::now();

    // Execute query with catalog and database context
    let query_result = mysql_client.query_raw(
        &sql,
        request.catalog.as_deref(),
        request.database.as_deref(),
    ).await;

    let execution_time_ms = start.elapsed().as_millis();

    match query_result {
        Ok((columns, data_rows)) => {
            let row_count = data_rows.len();

            Ok(Json(QueryExecuteResponse {
                columns,
                rows: data_rows,
                row_count,
                execution_time_ms,
            }))
        },
        Err(e) => Err(e),
    }
}

fn apply_query_limit(sql: &str, limit: i32) -> String {
    let sql_upper = sql.trim().to_uppercase();

    if sql_upper.contains("LIMIT") {
        return sql.to_string();
    }

    if sql_upper.starts_with("SELECT") {
        if sql_upper.contains("GET_QUERY_PROFILE")
            || sql_upper.contains("SHOW_PROFILE")
            || sql_upper.contains("EXPLAIN")
        {
            return sql.to_string();
        }

        format!("{} LIMIT {}", sql.trim().trim_end_matches(';'), limit)
    } else {
        sql.to_string()
    }
}
