// Data Statistics Service
// Purpose: Collect and cache expensive data statistics (database/table counts, top tables, etc.)
// Design Ref: CLUSTER_OVERVIEW_PLAN.md

use crate::models::Cluster;
use crate::services::{
    ClusterService, MaterializedViewService, MySQLClient, MySQLPoolManager, StarRocksClient,
};
use crate::utils::ApiResult;
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use utoipa::ToSchema;

/// Top table by size
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct TopTableBySize {
    pub database: String,
    pub table: String,
    pub size_bytes: i64,
    pub rows: Option<i64>,
}

/// Top table by access count
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct TopTableByAccess {
    pub database: String,
    pub table: String,
    pub access_count: i64,
    pub last_access: Option<String>,
}

/// Data statistics cache
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct DataStatistics {
    pub cluster_id: i64,
    pub updated_at: chrono::DateTime<Utc>,

    // Database/Table statistics
    pub database_count: i32,
    pub table_count: i32,
    pub total_data_size: i64,
    pub total_index_size: i64,

    // Top tables
    pub top_tables_by_size: Vec<TopTableBySize>,
    pub top_tables_by_access: Vec<TopTableByAccess>,

    // Materialized view statistics
    pub mv_total: i32,
    pub mv_running: i32,
    pub mv_failed: i32,
    pub mv_success: i32,

    // Schema change statistics
    pub schema_change_running: i32,
    pub schema_change_pending: i32,
    pub schema_change_finished: i32,
    pub schema_change_failed: i32,

    // Active users
    pub active_users_1h: i32,
    pub active_users_24h: i32,
    pub unique_users: Vec<String>,
}

#[derive(Clone)]
pub struct DataStatisticsService {
    db: SqlitePool,
    cluster_service: Arc<ClusterService>,
    mysql_pool_manager: Arc<MySQLPoolManager>,
}

impl DataStatisticsService {
    /// Create a new DataStatisticsService
    pub fn new(
        db: SqlitePool,
        cluster_service: Arc<ClusterService>,
        mysql_pool_manager: Arc<MySQLPoolManager>,
    ) -> Self {
        Self { db, cluster_service, mysql_pool_manager }
    }

    /// Collect and update data statistics for a cluster
    pub async fn update_statistics(&self, cluster_id: i64) -> ApiResult<DataStatistics> {
        tracing::info!("Updating data statistics for cluster {}", cluster_id);

        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Get MySQL connection pool
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        // Get database and table counts using MySQL queries
        let database_count = self.get_database_count_mysql(&mysql_client).await? as i32;
        let table_count = self.get_table_count_mysql(&mysql_client).await? as i32;

        // Get top tables by size (via MySQL client for detailed info)
        let top_tables_by_size = self.get_top_tables_by_size(&cluster, 20).await?;

        // Get top tables by access (from query history or audit logs)
        let top_tables_by_access = self.get_top_tables_by_access(&cluster, 20).await?;

        // Calculate total data size from all tables (not just top 20)
        let total_data_size = self.get_total_data_size_mysql(&mysql_client).await?;
        let total_index_size: i64 = 0; // INDEX_LENGTH is often NULL in StarRocks

        // Get materialized view statistics
        let (mv_total, mv_running, mv_failed, mv_success) =
            self.get_mv_statistics(&cluster).await?;

        // Get schema change statistics using MySQL
        let (
            schema_change_running,
            schema_change_pending,
            schema_change_finished,
            schema_change_failed,
        ) = self
            .get_schema_change_statistics_mysql(&mysql_client)
            .await?;

        // Get active users using MySQL
        let unique_users = self.get_active_users_mysql(&mysql_client).await?;
        let active_users_1h = unique_users.len() as i32; // Simplified: treat all as 1h active
        let active_users_24h = unique_users.len() as i32;

        let statistics = DataStatistics {
            cluster_id,
            updated_at: Utc::now(),
            database_count,
            table_count,
            total_data_size,
            total_index_size,
            top_tables_by_size,
            top_tables_by_access,
            mv_total,
            mv_running,
            mv_failed,
            mv_success,
            schema_change_running,
            schema_change_pending,
            schema_change_finished,
            schema_change_failed,
            active_users_1h,
            active_users_24h,
            unique_users,
        };

        // Save to cache
        self.save_statistics(&statistics).await?;

        tracing::info!(
            "Data statistics updated for cluster {}: {} databases, {} tables",
            cluster_id,
            database_count,
            table_count
        );

        Ok(statistics)
    }

    /// Get cached data statistics for a cluster
    pub async fn get_statistics(&self, cluster_id: i64) -> ApiResult<Option<DataStatistics>> {
        #[derive(sqlx::FromRow)]
        struct DataStatisticsRow {
            cluster_id: i64,
            updated_at: NaiveDateTime,
            database_count: i64,
            table_count: i64,
            total_data_size: i64,
            total_index_size: i64,
            top_tables_by_size: Option<String>,
            top_tables_by_access: Option<String>,
            mv_total: i64,
            mv_running: i64,
            mv_failed: i64,
            mv_success: i64,
            schema_change_running: i64,
            schema_change_pending: i64,
            schema_change_finished: i64,
            schema_change_failed: i64,
            active_users_1h: i64,
            active_users_24h: i64,
            unique_users: Option<String>,
        }

        let row: Option<DataStatisticsRow> = sqlx::query_as(
            r#"
            SELECT * FROM data_statistics
            WHERE cluster_id = ?
            "#,
        )
        .bind(cluster_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some(r) = row {
            let top_tables_by_size: Vec<TopTableBySize> = r
                .top_tables_by_size
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();

            let top_tables_by_access: Vec<TopTableByAccess> = r
                .top_tables_by_access
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();

            let unique_users: Vec<String> = r
                .unique_users
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();

            Ok(Some(DataStatistics {
                cluster_id: r.cluster_id,
                updated_at: r.updated_at.and_utc(),
                database_count: r.database_count as i32,
                table_count: r.table_count as i32,
                total_data_size: r.total_data_size,
                total_index_size: r.total_index_size,
                top_tables_by_size,
                top_tables_by_access,
                mv_total: r.mv_total as i32,
                mv_running: r.mv_running as i32,
                mv_failed: r.mv_failed as i32,
                mv_success: r.mv_success as i32,
                schema_change_running: r.schema_change_running as i32,
                schema_change_pending: r.schema_change_pending as i32,
                schema_change_finished: r.schema_change_finished as i32,
                schema_change_failed: r.schema_change_failed as i32,
                active_users_1h: r.active_users_1h as i32,
                active_users_24h: r.active_users_24h as i32,
                unique_users,
            }))
        } else {
            Ok(None)
        }
    }

    // ========================================
    // Internal helper methods
    // ========================================

    /// Get top tables by size
    async fn get_top_tables_by_size(
        &self,
        cluster: &Cluster,
        limit: usize,
    ) -> ApiResult<Vec<TopTableBySize>> {
        let pool = self.mysql_pool_manager.get_pool(cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        // Query information_schema.tables for table sizes
        // Note: Use DATA_LENGTH from StarRocks metadata, INDEX_LENGTH is often NULL
        let query = format!(
            r#"
            SELECT 
                TABLE_SCHEMA as database_name,
                TABLE_NAME as table_name,
                COALESCE(DATA_LENGTH, 0) as size_bytes,
                TABLE_ROWS as row_count
            FROM information_schema.tables
            WHERE TABLE_SCHEMA NOT IN ('information_schema', 'sys', 'mysql', '_statistics_')
              AND DATA_LENGTH > 0
            ORDER BY size_bytes DESC
            LIMIT {}
            "#,
            limit
        );

        let result = mysql_client.query(&query).await?;

        // Parse results
        let mut tables = Vec::new();
        for row in result {
            let database = row.get("database_name").and_then(|v| v.as_str());
            let table = row.get("table_name").and_then(|v| v.as_str());

            // Try to parse size_bytes - could be i64 or string
            let size_bytes = row.get("size_bytes").and_then(|v| v.as_i64()).or_else(|| {
                row.get("size_bytes")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
            });

            // Try to parse row_count - could be i64 or string
            let rows = row.get("row_count").and_then(|v| v.as_i64()).or_else(|| {
                row.get("row_count")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
            });

            if let (Some(db), Some(tbl), Some(size)) = (database, table, size_bytes) {
                tables.push(TopTableBySize {
                    database: db.to_string(),
                    table: tbl.to_string(),
                    size_bytes: size,
                    rows,
                });
            } else {
                tracing::debug!(
                    "Skipped row: db={:?}, table={:?}, size={:?}, rows={:?}",
                    database,
                    table,
                    size_bytes,
                    rows
                );
            }
        }

        tracing::info!("Retrieved {} top tables by size", tables.len());
        Ok(tables)
    }

    /// Get top tables by access count
    /// Note: This requires audit logs to be enabled in StarRocks
    async fn get_top_tables_by_access(
        &self,
        cluster: &Cluster,
        limit: usize,
    ) -> ApiResult<Vec<TopTableByAccess>> {
        let pool = self.mysql_pool_manager.get_pool(cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        // First try: Query with table name extraction from stmt
        // Extract table name after "FROM" keyword, clean and lowercase
        let query_with_table = format!(
            r#"
            SELECT 
                db as database_name,
                LOWER(
                    TRIM(
                        REPLACE(
                            REPLACE(
                                SUBSTRING_INDEX(
                                    SUBSTRING_INDEX(
                                        SUBSTRING_INDEX(stmt, 'FROM ', -1),
                                        ' ', 
                                        1
                                    ),
                                    '.',
                                    -1
                                ),
                                '`',
                                ''
                            ),
                            ')',
                            ''
                        )
                    )
                ) as table_name,
                COUNT(*) as access_count
            FROM starrocks_audit_db__.starrocks_audit_tbl__
            WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 3 DAY)
                AND db NOT IN ('information_schema', '_statistics_', '', 'sys', 'starrocks_audit_db__', 'recycle_dw')
                AND UPPER(stmt) LIKE '%FROM %'
            GROUP BY db, table_name
            HAVING table_name NOT LIKE '%select%'
                AND table_name NOT LIKE '%(%'
                AND LENGTH(table_name) > 0
                AND LENGTH(table_name) < 100
            ORDER BY access_count DESC
            LIMIT {}
            "#,
            limit
        );

        tracing::debug!("Querying top tables by access with table name extraction");

        match mysql_client.query(&query_with_table).await {
            Ok(results) => {
                let tables = results
                    .into_iter()
                    .filter_map(|row| {
                        let database = row.get("database_name").and_then(|v| v.as_str())?;
                        let table = row.get("table_name").and_then(|v| v.as_str())?;

                        // Try to parse access_count - could be i64 or string
                        let access_count = row
                            .get("access_count")
                            .and_then(|v| v.as_i64())
                            .or_else(|| {
                                row.get("access_count")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.parse::<i64>().ok())
                            })?;

                        Some(TopTableByAccess {
                            database: database.to_string(),
                            table: table.to_string(),
                            access_count,
                            last_access: None,
                        })
                    })
                    .collect::<Vec<_>>();

                tracing::info!("Retrieved {} top tables by access", tables.len());
                Ok(tables)
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to query audit logs with table extraction: {}. Falling back to database-level stats.",
                    e
                );

                // Fallback: database-level statistics
                let query_db_only = format!(
                    r#"
                    SELECT 
                        db as database_name,
                        COUNT(*) as access_count
                    FROM starrocks_audit_db__.starrocks_audit_tbl__
                    WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 3 DAY)
                        AND db NOT IN ('information_schema', '_statistics_', '', 'sys', 'starrocks_audit_db__', 'recycle_dw')
                    GROUP BY db
                    ORDER BY access_count DESC
                    LIMIT {}
                    "#,
                    limit
                );

                match mysql_client.query(&query_db_only).await {
                    Ok(results) => {
                        let tables = results
                            .into_iter()
                            .filter_map(|row| {
                                let database = row.get("database_name").and_then(|v| v.as_str())?;
                                let access_count = row
                                    .get("access_count")
                                    .and_then(|v| v.as_i64())
                                    .or_else(|| {
                                        row.get("access_count")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| s.parse::<i64>().ok())
                                    })?;

                                Some(TopTableByAccess {
                                    database: database.to_string(),
                                    table: "(所有表)".to_string(), // Database-level fallback
                                    access_count,
                                    last_access: None,
                                })
                            })
                            .collect::<Vec<_>>();

                        tracing::info!(
                            "Retrieved {} top databases by access (fallback)",
                            tables.len()
                        );
                        Ok(tables)
                    },
                    Err(e) => {
                        tracing::warn!(
                            "Failed to query audit logs (fallback): {}. Returning empty list.",
                            e
                        );
                        Ok(Vec::new())
                    },
                }
            },
        }
    }

    /// Get materialized view statistics
    async fn get_mv_statistics(&self, cluster: &Cluster) -> ApiResult<(i32, i32, i32, i32)> {
        // Create MV service for this cluster
        let pool = self.mysql_pool_manager.get_pool(cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);
        let mv_service = MaterializedViewService::new(mysql_client);

        // Get all materialized views
        let mvs = mv_service.list_materialized_views(None).await?;

        let mv_total = mvs.len() as i32;

        // Count by state
        // Note: StarRocks MV states:
        // - is_active=true: MV is active and can be used
        // - is_active=false: MV is inactive (failed or paused)
        // - refresh_type: MANUAL/ASYNC
        let mv_active = mvs.iter().filter(|mv| mv.is_active).count() as i32;
        let mv_inactive = mvs.iter().filter(|mv| !mv.is_active).count() as i32;

        // For now, consider active MVs as "success" and inactive as "failed"
        // In the future, we could query task history for more accurate stats
        let mv_success = mv_active;
        let mv_failed = mv_inactive;
        let mv_running = 0; // Would need to query running tasks

        tracing::debug!(
            "MV statistics: total={}, active={}, inactive={}",
            mv_total,
            mv_active,
            mv_inactive
        );

        Ok((mv_total, mv_running, mv_failed, mv_success))
    }

    /// Get schema change statistics
    #[allow(dead_code)]
    async fn get_schema_change_statistics(
        &self,
        client: &StarRocksClient,
    ) -> ApiResult<(i32, i32, i32, i32)> {
        let changes = client.get_schema_changes().await?;

        let running = changes.iter().filter(|c| c.state == "RUNNING").count() as i32;
        let pending = changes.iter().filter(|c| c.state == "PENDING").count() as i32;
        let finished = changes.iter().filter(|c| c.state == "FINISHED").count() as i32;
        let failed = changes
            .iter()
            .filter(|c| c.state == "CANCELLED" || c.state == "FAILED")
            .count() as i32;

        Ok((running, pending, finished, failed))
    }

    /// Save statistics to cache
    async fn save_statistics(&self, stats: &DataStatistics) -> ApiResult<()> {
        let top_tables_by_size_json = serde_json::to_string(&stats.top_tables_by_size)?;
        let top_tables_by_access_json = serde_json::to_string(&stats.top_tables_by_access)?;
        let unique_users_json = serde_json::to_string(&stats.unique_users)?;

        sqlx::query(
            r#"
            INSERT INTO data_statistics (
                cluster_id, updated_at,
                database_count, table_count, total_data_size, total_index_size,
                top_tables_by_size, top_tables_by_access,
                mv_total, mv_running, mv_failed, mv_success,
                schema_change_running, schema_change_pending, 
                schema_change_finished, schema_change_failed,
                active_users_1h, active_users_24h, unique_users
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(cluster_id) DO UPDATE SET
                updated_at = excluded.updated_at,
                database_count = excluded.database_count,
                table_count = excluded.table_count,
                total_data_size = excluded.total_data_size,
                total_index_size = excluded.total_index_size,
                top_tables_by_size = excluded.top_tables_by_size,
                top_tables_by_access = excluded.top_tables_by_access,
                mv_total = excluded.mv_total,
                mv_running = excluded.mv_running,
                mv_failed = excluded.mv_failed,
                mv_success = excluded.mv_success,
                schema_change_running = excluded.schema_change_running,
                schema_change_pending = excluded.schema_change_pending,
                schema_change_finished = excluded.schema_change_finished,
                schema_change_failed = excluded.schema_change_failed,
                active_users_1h = excluded.active_users_1h,
                active_users_24h = excluded.active_users_24h,
                unique_users = excluded.unique_users
            "#,
        )
        .bind(stats.cluster_id)
        .bind(stats.updated_at)
        .bind(stats.database_count)
        .bind(stats.table_count)
        .bind(stats.total_data_size)
        .bind(stats.total_index_size)
        .bind(top_tables_by_size_json)
        .bind(top_tables_by_access_json)
        .bind(stats.mv_total)
        .bind(stats.mv_running)
        .bind(stats.mv_failed)
        .bind(stats.mv_success)
        .bind(stats.schema_change_running)
        .bind(stats.schema_change_pending)
        .bind(stats.schema_change_finished)
        .bind(stats.schema_change_failed)
        .bind(stats.active_users_1h)
        .bind(stats.active_users_24h)
        .bind(unique_users_json)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Get database count using MySQL query
    async fn get_database_count_mysql(&self, mysql_client: &MySQLClient) -> ApiResult<i64> {
        let sql = "SELECT COUNT(*) as count FROM information_schema.schemata WHERE schema_name NOT IN ('information_schema', '_statistics_')";
        let results = mysql_client.query(sql).await?;

        if let Some(first_row) = results.first()
            && let Some(count_str) = first_row.get("count").and_then(|v| v.as_str())
        {
            return Ok(count_str.parse::<i64>().unwrap_or(0));
        }
        Ok(0)
    }

    /// Get table count using MySQL query
    async fn get_table_count_mysql(&self, mysql_client: &MySQLClient) -> ApiResult<i64> {
        let sql = "SELECT COUNT(*) as count FROM information_schema.tables WHERE table_schema NOT IN ('information_schema', '_statistics_')";
        let results = mysql_client.query(sql).await?;

        if let Some(first_row) = results.first()
            && let Some(count_str) = first_row.get("count").and_then(|v| v.as_str())
        {
            return Ok(count_str.parse::<i64>().unwrap_or(0));
        }
        Ok(0)
    }

    /// Get total data size from all tables using MySQL query
    async fn get_total_data_size_mysql(&self, mysql_client: &MySQLClient) -> ApiResult<i64> {
        let sql = "SELECT SUM(COALESCE(DATA_LENGTH, 0)) as total_size FROM information_schema.tables WHERE table_schema NOT IN ('information_schema', '_statistics_')";
        let (columns, rows) = mysql_client.query_raw(sql, None, None).await?;

        if let Some(total_idx) = columns
            .iter()
            .position(|col| col.to_lowercase() == "total_size")
            && let Some(row) = rows.first()
            && let Some(size_str) = row.get(total_idx)
        {
            return Ok(size_str.parse::<i64>().unwrap_or(0));
        }
        Ok(0)
    }

    /// Get schema change statistics using SHOW ALTER TABLE
    async fn get_schema_change_statistics_mysql(
        &self,
        mysql_client: &MySQLClient,
    ) -> ApiResult<(i32, i32, i32, i32)> {
        let sql = "SHOW ALTER TABLE";
        let (_columns, rows) = mysql_client.query_raw(sql, None, None).await.unwrap_or_default();

        let mut running = 0;
        let mut pending = 0;
        let mut finished = 0;
        let mut failed = 0;

        for row in rows {
            // Assuming State is typically the second column, but we search for it
            if !row.is_empty() {
                let state_str = &row[row.len() - 1]; // Usually the last column
                match state_str.to_uppercase().as_str() {
                    "RUNNING" => running += 1,
                    "PENDING" | "WAITING_TXN" => pending += 1,
                    "FINISHED" => finished += 1,
                    "CANCELLED" | "FAILED" => failed += 1,
                    _ => {},
                }
            }
        }

        Ok((running, pending, finished, failed))
    }

    /// Get active users using SHOW PROCESSLIST
    async fn get_active_users_mysql(&self, mysql_client: &MySQLClient) -> ApiResult<Vec<String>> {
        let sql = "SHOW PROCESSLIST";
        let (columns, rows) = mysql_client.query_raw(sql, None, None).await.unwrap_or_default();

        let mut unique_users: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Find the User column index
        if let Some(user_idx) = columns.iter().position(|col| col.to_lowercase() == "user") {
            for row in rows {
                if let Some(user) = row.get(user_idx) {
                    // Filter out system users
                    if user != "root" && !user.is_empty() {
                        unique_users.insert(user.clone());
                    }
                }
            }
        }

        Ok(unique_users.into_iter().collect())
    }
}
