use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;

use crate::models::starrocks::{QueryHistoryItem, QueryHistoryResponse};
use crate::services::mysql_client::MySQLClient;
use crate::utils::error::ApiResult;

#[derive(Deserialize)]
pub struct HistoryQueryParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// offset for pagination
    #[serde(default = "default_offset")]
    pub offset: i64,
    /// search keyword for query_id, sql_statement, or user
    pub keyword: Option<String>,
    /// start time filter
    pub start_time: Option<String>,
    /// end time filter
    pub end_time: Option<String>,
}

fn default_limit() -> i64 {
    10
}
fn default_offset() -> i64 {
    0
}

#[utoipa::path(
    get,
    path = "/api/clusters/queries/history",
    responses((status = 200, description = "Finished query list with pagination", body = QueryHistoryResponse)),
    security(("bearer_auth" = [])),
    tag = "Queries"
)]
pub async fn list_query_history(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    axum::extract::Query(params): axum::extract::Query<HistoryQueryParams>,
) -> ApiResult<Json<QueryHistoryResponse>> {
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
    let mysql = MySQLClient::from_pool(pool);

    let limit = params.limit;
    let offset = params.offset;
    let keyword = params.keyword.as_deref().unwrap_or("");
    let start_time = params.start_time.as_deref();
    let end_time = params.end_time.as_deref();

    // Build WHERE conditions
    let mut where_conditions = vec![
        "isQuery = 1".to_string(),
        "`timestamp` >= DATE_SUB(NOW(), INTERVAL 7 DAY)".to_string(),
    ];

    // Add keyword search if provided
    if !keyword.is_empty() {
        where_conditions.push(format!(
            "(`queryId` LIKE '%{}%' OR `stmt` LIKE '%{}%' OR `user` LIKE '%{}%')",
            keyword.replace('\'', "''"), // Escape single quotes
            keyword.replace('\'', "''"),
            keyword.replace('\'', "''")
        ));
    }

    // Add time range filters if provided
    if let Some(start) = start_time {
        where_conditions.push(format!("`timestamp` >= '{}'", start));
    }
    if let Some(end) = end_time {
        where_conditions.push(format!("`timestamp` <= '{}'", end));
    }

    let where_clause = where_conditions.join(" AND ");

    // Get audit table name from config
    let audit_table = state.audit_config.full_table_name();

    // First, get the total count (required for ng2-smart-table pagination)
    let count_sql = format!(
        r#"
        SELECT COUNT(*) as total
        FROM {}
        WHERE {}
    "#,
        audit_table, where_clause
    );

    tracing::info!("Fetching total count for cluster {}", cluster.id);
    let (_, count_rows) = mysql.query_raw(&count_sql).await.map_err(|e| {
        tracing::error!("Failed to query count: {:?}", e);
        e
    })?;

    let total: i64 = if let Some(row) = count_rows.first() {
        if let Some(count_str) = row.first() {
            count_str.parse::<i64>().unwrap_or_else(|_| {
                tracing::warn!("Could not parse count result, defaulting to 0");
                0i64
            })
        } else {
            0i64
        }
    } else {
        0i64
    };

    tracing::info!("Total history records: {}", total);

    // Then fetch the paginated data
    let sql = format!(
        r#"
        SELECT 
            queryId,
            `user`,
            COALESCE(`db`, '') AS db,
            `stmt`,
            `queryType`,
            `timestamp` AS start_time,
            `queryTime` AS total_ms,
            `state`,
            COALESCE(`resourceGroup`, '') AS warehouse
        FROM {}
        WHERE {}
        ORDER BY `timestamp` DESC
        LIMIT {} OFFSET {}
    "#,
        audit_table, where_clause, limit, offset
    );

    tracing::info!(
        "Fetching query history for cluster {} (limit: {}, offset: {})",
        cluster.id,
        limit,
        offset
    );
    let (columns, rows) = mysql.query_raw(&sql).await.map_err(|e| {
        tracing::error!("Failed to query audit table: {:?}", e);
        e
    })?;
    tracing::info!("Fetched {} history records", rows.len());

    // Build column index map for easier access
    let mut col_idx = std::collections::HashMap::new();
    for (i, col) in columns.iter().enumerate() {
        col_idx.insert(col.clone(), i);
    }

    let mut items: Vec<QueryHistoryItem> = Vec::with_capacity(rows.len());
    for row in &rows {
        let query_id = col_idx
            .get("queryId")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_default();
        let user = col_idx
            .get("user")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_default();
        let db = col_idx
            .get("db")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_default();
        let stmt = col_idx
            .get("stmt")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_default();
        let qtype = col_idx
            .get("queryType")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_else(|| "Query".to_string());
        let start_time = col_idx
            .get("start_time")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_default();
        let total_ms_raw = col_idx
            .get("total_ms")
            .and_then(|&i| row.get(i))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let state = col_idx
            .get("state")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_default();
        let warehouse = col_idx
            .get("warehouse")
            .and_then(|&i| row.get(i))
            .cloned()
            .unwrap_or_default();

        items.push(QueryHistoryItem {
            query_id,
            user,
            default_db: db,
            sql_statement: stmt,
            query_type: qtype,
            start_time,
            end_time: String::new(), // Can be calculated on frontend if needed
            total_ms: total_ms_raw,
            query_state: state,
            warehouse,
        });
    }

    let page = (offset / limit) + 1;

    Ok(Json(QueryHistoryResponse { data: items, total, page, page_size: limit }))
}
