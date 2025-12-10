// Audit Log Service
// Purpose: Query and analyze StarRocks audit logs for access patterns and slow queries
// Design Ref: AUDIT_LOG_FEATURES.md

#![allow(dead_code)]

use crate::config::AuditLogConfig;
use crate::models::Cluster;
use crate::services::{MySQLClient, MySQLPoolManager};
use crate::utils::ApiResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// Top table by access count (from audit logs)
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct TopTableByAccess {
    pub database: String,
    pub table: String,
    pub access_count: i64,
    pub last_access: Option<String>,
    pub unique_users: i32,
}

/// Slow query information
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct SlowQuery {
    pub query_id: String,
    pub user: String,
    pub database: String,
    pub duration_ms: i64,
    pub scan_rows: Option<i64>,
    pub scan_bytes: Option<i64>,
    pub return_rows: Option<i64>,
    pub cpu_cost_ms: Option<i64>,
    pub mem_cost_bytes: Option<i64>,
    pub timestamp: String,
    pub state: String,
    pub query_preview: String, // First 200 characters
}

pub struct AuditLogService {
    mysql_pool_manager: Arc<MySQLPoolManager>,
    audit_config: AuditLogConfig,
}

impl AuditLogService {
    pub fn new(mysql_pool_manager: Arc<MySQLPoolManager>, audit_config: AuditLogConfig) -> Self {
        Self {
            mysql_pool_manager,
            audit_config,
        }
    }

    /// Get top tables by access count
    /// 
    /// This queries the audit log to find the most frequently accessed tables.
    /// 
    /// # Arguments
    /// * `cluster` - The StarRocks cluster
    /// * `hours` - Time window in hours (default: 24)
    /// * `limit` - Maximum number of results (default: 20)
    pub async fn get_top_tables_by_access(
        &self,
        cluster: &Cluster,
        hours: i32,
        limit: usize,
    ) -> ApiResult<Vec<TopTableByAccess>> {
        let pool = self.mysql_pool_manager.get_pool(cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);
        let audit_table = self.audit_config.full_table_name();
        let audit_table_filter = &self.audit_config.table;
        let query = format!(
            r#"
            SELECT 
                COALESCE(NULLIF(`catalog`, 'default_catalog'), '') as catalog,
                COALESCE(NULLIF(`db`, ''), '') as database,
                -- Extract full table reference from stmt (handles catalog.db.table format)
                TRIM(BOTH '`' FROM 
                    REGEXP_REPLACE(
                        REGEXP_REPLACE(
                            `stmt`, 
                            '.*\\b(?:FROM|JOIN|INTO|TABLE)\\s+(`?[a-zA-Z0-9_]+`?(?:\\.[a-zA-Z0-9_]+){{1,2}}|`?[a-zA-Z0-9_]+`?).*', 
                            '$1'
                        ),
                        '`', ''
                    )
                ) as full_table_name,
                COUNT(*) as access_count,
                MAX(`timestamp`) as last_access,
                COUNT(DISTINCT `user`) as unique_users
            FROM {audit_table}
            WHERE `timestamp` >= DATE_SUB(NOW(), INTERVAL {hours} HOUR)
                AND isQuery = 1
                AND `state` = 'EOF'
                AND `queryType` IN ('SELECT', 'INSERT', 'UPDATE', 'DELETE')
                AND `catalog` != ''
                AND (`db` != 'information_schema' OR `db` IS NULL)
                AND (`db` != '_statistics_' OR `db` IS NULL)
                AND LOWER(`stmt`) NOT LIKE '%{audit_table_filter}%'
            GROUP BY catalog, database, full_table_name
            HAVING full_table_name != ''
                AND full_table_name NOT LIKE '%(%'
                AND full_table_name NOT LIKE '%SELECT%'
                AND full_table_name NOT LIKE '%WHERE%'
                AND full_table_name NOT LIKE '%GROUP%'
            ORDER BY access_count DESC
            LIMIT {limit}
            "#,
        );
        
        tracing::debug!("Querying top tables by access: hours={}, limit={}", hours, limit);
        
        let (columns, rows) = mysql_client.query_raw(&query).await?;
        
        // Build column index map
        let mut col_idx = std::collections::HashMap::new();
        for (i, col) in columns.iter().enumerate() {
            col_idx.insert(col.clone(), i);
        }
        
        let mut tables = Vec::new();
        for row in rows {
            if let (Some(full_table_name), Some(access_count_str)) = (
                col_idx.get("full_table_name").and_then(|&i| row.get(i)),
                col_idx.get("access_count").and_then(|&i| row.get(i)),
            ) {
                let access_count = access_count_str.parse::<i64>().unwrap_or(0);
                let last_access = col_idx
                    .get("last_access")
                    .and_then(|&i| row.get(i))
                    .cloned();
                
                let unique_users = col_idx
                    .get("unique_users")
                    .and_then(|&i| row.get(i))
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0);
                
                let catalog = col_idx
                    .get("catalog")
                    .and_then(|&i| row.get(i))
                    .unwrap_or("");
                
                let db_field = col_idx
                    .get("database")
                    .and_then(|&i| row.get(i))
                    .unwrap_or("");
                
                // Parse full_table_name: could be "table", "db.table", or "catalog.db.table"
                let parts: Vec<&str> = full_table_name.split('.').collect();
                let (final_db, final_table) = match parts.len() {
                    3 => {
                        // catalog.database.table format (external catalog)
                        (format!("{}.{}", parts[0], parts[1]), parts[2].to_string())
                    },
                    2 => {
                        // database.table format
                        if !catalog.is_empty() {
                            // External catalog: catalog.database
                            (format!("{}.{}", catalog, parts[0]), parts[1].to_string())
                        } else {
                            // Default catalog: database.table
                            (parts[0].to_string(), parts[1].to_string())
                        }
                    },
                    1 => {
                        // Just table name, use db field or empty
                        (db_field.to_string(), parts[0].to_string())
                    },
                    _ => continue, // Invalid format, skip
                };
                
                tables.push(TopTableByAccess {
                    database: final_db,
                    table: final_table,
                    access_count,
                    last_access,
                    unique_users,
                });
            }
        }
        
        tracing::info!(
            "Found {} top tables by access ({}h window)",
            tables.len(),
            hours
        );
        
        Ok(tables)
    }

    /// Get slow queries
    /// 
    /// This queries the audit log to find slow-running queries.
    /// 
    /// # Arguments
    /// * `cluster` - The StarRocks cluster
    /// * `hours` - Time window in hours (default: 24)
    /// * `min_duration_ms` - Minimum query duration in milliseconds (default: 1000)
    /// * `limit` - Maximum number of results (default: 20)
    pub async fn get_slow_queries(
        &self,
        cluster: &Cluster,
        hours: i32,
        min_duration_ms: i64,
        limit: usize,
    ) -> ApiResult<Vec<SlowQuery>> {
        let pool = self.mysql_pool_manager.get_pool(cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);
        let audit_table = self.audit_config.full_table_name();
        
        // Query audit logs for slow queries
        let query = format!(
            r#"
            SELECT 
                queryId as query_id,
                `user`,
                COALESCE(`db`, '') as `database`,
                `queryTime` as duration_ms,
                `scanRows` as scan_rows,
                `scanBytes` as scan_bytes,
                `returnRows` as return_rows,
                `cpuCostNs` / 1000000 as cpu_cost_ms,
                `memCostBytes` as mem_cost_bytes,
                `timestamp`,
                `state`,
                LEFT(`stmt`, 200) as query_preview
            FROM {audit_table}
            WHERE `timestamp` >= DATE_SUB(NOW(), INTERVAL {hours} HOUR)
                AND `queryTime` >= {min_duration_ms}
                AND isQuery = 1
                AND `state` = 'EOF'
            ORDER BY `queryTime` DESC
            LIMIT {limit}
            "#,
        );
        
        tracing::debug!(
            "Querying slow queries: hours={}, min_duration={}ms, limit={}",
            hours,
            min_duration_ms,
            limit
        );
        
        let (columns, rows) = mysql_client.query_raw(&query).await?;
        
        // Build column index map
        let mut col_idx = std::collections::HashMap::new();
        for (i, col) in columns.iter().enumerate() {
            col_idx.insert(col.clone(), i);
        }
        
        let mut slow_queries = Vec::new();
        for row in rows {
            if let (Some(query_id), Some(user), Some(database), Some(duration_ms_str)) = (
                col_idx.get("query_id").and_then(|&i| row.get(i)),
                col_idx.get("user").and_then(|&i| row.get(i)),
                col_idx.get("database").and_then(|&i| row.get(i)),
                col_idx.get("duration_ms").and_then(|&i| row.get(i)),
            ) {
                let duration_ms = duration_ms_str.parse::<i64>().unwrap_or(0);
                
                let scan_rows = col_idx
                    .get("scan_rows")
                    .and_then(|&i| row.get(i))
                    .and_then(|s| s.parse::<i64>().ok());
                
                let scan_bytes = col_idx
                    .get("scan_bytes")
                    .and_then(|&i| row.get(i))
                    .and_then(|s| s.parse::<i64>().ok());
                
                let return_rows = col_idx
                    .get("return_rows")
                    .and_then(|&i| row.get(i))
                    .and_then(|s| s.parse::<i64>().ok());
                
                let cpu_cost_ms = col_idx
                    .get("cpu_cost_ms")
                    .and_then(|&i| row.get(i))
                    .and_then(|s| s.parse::<i64>().ok());
                
                let mem_cost_bytes = col_idx
                    .get("mem_cost_bytes")
                    .and_then(|&i| row.get(i))
                    .and_then(|s| s.parse::<i64>().ok());
                
                let timestamp = col_idx
                    .get("timestamp")
                    .and_then(|&i| row.get(i))
                    .cloned()
                    .unwrap_or_default();
                
                let state = col_idx
                    .get("state")
                    .and_then(|&i| row.get(i))
                    .cloned()
                    .unwrap_or_else(|| "UNKNOWN".to_string());
                
                let query_preview = col_idx
                    .get("query_preview")
                    .and_then(|&i| row.get(i))
                    .cloned()
                    .unwrap_or_default();
                
                slow_queries.push(SlowQuery {
                    query_id: query_id.to_string(),
                    user: user.to_string(),
                    database: database.to_string(),
                    duration_ms,
                    scan_rows,
                    scan_bytes,
                    return_rows,
                    cpu_cost_ms,
                    mem_cost_bytes,
                    timestamp,
                    state,
                    query_preview,
                });
            }
        }
        
        tracing::info!(
            "Found {} slow queries (>{}ms, {}h window)",
            slow_queries.len(),
            min_duration_ms,
            hours
        );
        
        Ok(slow_queries)
    }
}

