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
use crate::models::{
    CatalogWithDatabases, CatalogsWithDatabasesResponse, Query, QueryExecuteRequest,
    QueryExecuteResponse, SingleQueryResult,
};
use crate::services::StarRocksClient;
use crate::services::mysql_client::MySQLClient;
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

    let (_, rows) = mysql_client.query_raw("SHOW CATALOGS").await?;

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
        // First switch to the catalog, then show databases
        // Try without backticks - StarRocks may not support them for catalog names
        let use_catalog_sql = format!("USE CATALOG {}", catalog_name);
        if let Err(e) = mysql_client.execute(&use_catalog_sql).await {
            tracing::warn!("Failed to switch to catalog {}: {}", catalog_name, e);
            // Continue anyway, might be using default catalog
        }
    }

    // Execute SHOW DATABASES
    let (_, rows) = mysql_client.query_raw("SHOW DATABASES").await?;

    let mut databases = Vec::new();
    for row in rows {
        if let Some(db_name) = row.first() {
            let name = db_name.trim().to_string();
            // Skip system databases
            if !name.is_empty() && name != "information_schema" && name != "_statistics_" {
                databases.push(name);
            }
        }
    }

    tracing::debug!("Found {} databases via MySQL client", databases.len());
    Ok(Json(databases))
}

// Get list of tables within a database (optional catalog) using MySQL client
#[utoipa::path(
    get,
    path = "/api/clusters/tables",
    params(
        ("catalog" = Option<String>, Query, description = "Catalog name (optional)"),
        ("database" = String, Query, description = "Database name")
    ),
    responses(
        (status = 200, description = "List of tables", body = Vec<String>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Queries"
)]
pub async fn list_tables(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<Vec<String>>> {
    let cluster = state.cluster_service.get_active_cluster().await?;

    // Use MySQL client
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // Database is required to list tables
    let database_name = match params.get("database") {
        Some(name) if !name.trim().is_empty() => name.trim().to_string(),
        _ => {
            tracing::warn!("Database parameter missing when listing tables");
            return Ok(Json(Vec::new()));
        },
    };

    let mut session = mysql_client.create_session().await?;

    if let Some(catalog_name) = params.get("catalog") {
        session.use_catalog(catalog_name).await?;
    }

    session.use_database(&database_name).await?;

    let (_, rows, _) = session.execute("SHOW TABLES").await?;

    let mut tables = Vec::new();
    for row in rows {
        if let Some(table_name) = row.first() {
            let name = table_name.trim().to_string();
            if !name.is_empty() {
                tables.push(name);
            }
        }
    }

    tracing::debug!("Database {} returned {} tables via MySQL client", database_name, tables.len());

    Ok(Json(tables))
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
    let (_, catalog_rows) = mysql_client.query_raw("SHOW CATALOGS").await?;

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

        // Get databases for this catalog
        let (_, db_rows) = match mysql_client.query_raw("SHOW DATABASES").await {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("Failed to get databases for catalog {}: {}", catalog_name, e);
                catalogs.push(CatalogWithDatabases {
                    catalog: catalog_name.clone(),
                    databases: Vec::new(),
                });
                continue;
            },
        };

        let mut databases = Vec::new();
        for row in db_rows {
            if let Some(db_name) = row.first() {
                let name = db_name.trim().to_string();
                // Skip system databases
                if !name.is_empty() && name != "information_schema" && name != "_statistics_" {
                    databases.push(name);
                }
            }
        }

        tracing::debug!("Catalog {} has {} databases", catalog_name, databases.len());

        catalogs.push(CatalogWithDatabases { catalog: catalog_name.clone(), databases });
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
    let client = StarRocksClient::new(cluster);
    let queries = client.get_queries().await?;
    Ok(Json(queries))
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

    // Parse SQL statements first (split by semicolon, handling simple cases)
    let sql_statements = parse_sql_statements(&request.sql);

    // Limit to maximum 5 statements
    let sql_statements: Vec<String> = sql_statements.into_iter().take(5).collect();

    // If no statements to execute, return early
    if sql_statements.is_empty() {
        return Ok(Json(QueryExecuteResponse { results: Vec::new(), total_execution_time_ms: 0 }));
    }

    // CRITICAL: Create a session with a dedicated connection
    // This ensures USE CATALOG/DATABASE state persists across all queries
    let mut session = mysql_client.create_session().await?;

    // Execute USE CATALOG only once on the session's connection
    if let Some(cat) = request.catalog.as_ref().filter(|c| !c.is_empty()) {
        session.use_catalog(cat).await?;
    }

    // Execute USE DATABASE only once on the session's connection
    if let Some(db) = request.database.as_ref().filter(|d| !d.is_empty()) {
        session.use_database(db).await?;
    }

    let total_start = Instant::now();
    let mut results = Vec::new();

    // Execute each SQL statement sequentially on the SAME connection
    for sql in sql_statements {
        if sql.is_empty() {
            continue;
        }

        let sql_with_limit = apply_query_limit(&sql, request.limit.unwrap_or(1000));

        // Execute query on the session's connection that has persistent database context
        // Use execute to get accurate SQL execution time (excluding data processing)
        let query_result = session.execute(&sql_with_limit).await;

        match query_result {
            Ok((columns, data_rows, execution_time_ms)) => {
                let row_count = data_rows.len();
                results.push(SingleQueryResult {
                    sql,
                    columns,
                    rows: data_rows,
                    row_count,
                    execution_time_ms,
                    success: true,
                    error: None,
                });
            },
            Err(e) => {
                results.push(SingleQueryResult {
                    sql,
                    columns: Vec::new(),
                    rows: Vec::new(),
                    row_count: 0,
                    execution_time_ms: 0, // No timing available for errors
                    success: false,
                    error: Some(e.to_string()),
                });
            },
        }
    }

    let total_execution_time_ms = total_start.elapsed().as_millis();

    // Session's connection will be automatically returned to pool when session is dropped

    Ok(Json(QueryExecuteResponse { results, total_execution_time_ms }))
}

// Simple SQL statement parser - splits by semicolon, ignoring those in single/double quotes
fn parse_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current.push(ch);
            },
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current.push(ch);
            },
            ';' if !in_single_quote && !in_double_quote => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    statements.push(trimmed.to_string());
                }
                current.clear();
            },
            _ => {
                current.push(ch);
            },
        }
    }

    // Add the last statement if exists
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}

fn apply_query_limit(sql: &str, limit: i32) -> String {
    let trimmed = sql.trim();
    let sql_upper = trimmed.to_uppercase();

    // Return original SQL if it already has LIMIT
    if sql_upper.contains("LIMIT") {
        return trimmed.to_string();
    }

    // Only add LIMIT to SELECT queries that don't contain special keywords
    if sql_upper.starts_with("SELECT") {
        if sql_upper.contains("GET_QUERY_PROFILE")
            || sql_upper.contains("SHOW_PROFILE")
            || sql_upper.contains("EXPLAIN")
        {
            return trimmed.to_string();
        }

        // Add LIMIT to SELECT query
        let sql_without_semicolon = trimmed.trim_end_matches(';');
        format!("{} LIMIT {}", sql_without_semicolon, limit)
    } else {
        trimmed.to_string()
    }
}
