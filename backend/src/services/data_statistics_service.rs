// Data Statistics Service
// Purpose: Collect and cache expensive data statistics (database/table counts, top tables, etc.)
// Design Ref: CLUSTER_OVERVIEW_PLAN.md

use crate::models::Cluster;
use crate::services::{AuditLogService, ClusterService, MaterializedViewService, MySQLClient, MySQLPoolManager, StarRocksClient};
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
        Self {
            db,
            cluster_service,
            mysql_pool_manager,
        }
    }

    /// Collect and update data statistics for a cluster
    pub async fn update_statistics(&self, cluster_id: i64) -> ApiResult<DataStatistics> {
        tracing::info!("Updating data statistics for cluster {}", cluster_id);
        
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;
        let client = StarRocksClient::new(cluster.clone());
        
        // Get database and table counts
        let database_count = client.get_database_count().await? as i32;
        let table_count = client.get_total_table_count().await? as i32;
        
        // Get top tables by size (via MySQL client for detailed info)
        let top_tables_by_size = self.get_top_tables_by_size(&cluster, 20).await?;
        
        // Get top tables by access (from query history or audit logs)
        let top_tables_by_access = self.get_top_tables_by_access(&cluster, 20).await?;
        
        // Calculate total data size
        let total_data_size: i64 = top_tables_by_size.iter().map(|t| t.size_bytes).sum();
        let total_index_size: i64 = 0; // TODO: Calculate from table stats
        
        // Get materialized view statistics
        let (mv_total, mv_running, mv_failed, mv_success) = 
            self.get_mv_statistics(&cluster).await?;
        
        // Get schema change statistics
        let (schema_change_running, schema_change_pending, schema_change_finished, schema_change_failed) =
            self.get_schema_change_statistics(&client).await?;
        
        // Get active users
        let unique_users = client.get_active_users().await?;
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
            "#
        )
        .bind(cluster_id)
        .fetch_optional(&self.db)
        .await?;
        
        if let Some(r) = row {
            let top_tables_by_size: Vec<TopTableBySize> = r.top_tables_by_size
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            
            let top_tables_by_access: Vec<TopTableByAccess> = r.top_tables_by_access
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            
            let unique_users: Vec<String> = r.unique_users
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
        let query = format!(
            r#"
            SELECT 
                TABLE_SCHEMA as `database`,
                TABLE_NAME as `table`,
                COALESCE(DATA_LENGTH, 0) + COALESCE(INDEX_LENGTH, 0) as size_bytes,
                TABLE_ROWS as rows
            FROM information_schema.tables
            WHERE TABLE_SCHEMA NOT IN ('information_schema', 'sys', 'mysql')
            ORDER BY size_bytes DESC
            LIMIT {}
            "#,
            limit
        );
        
        let result = mysql_client.query(&query).await?;
        
        // Parse results
        let mut tables = Vec::new();
        for row in result {
            if let (Some(database), Some(table), Some(size_bytes)) = (
                row.get("database").and_then(|v| v.as_str()),
                row.get("table").and_then(|v| v.as_str()),
                row.get("size_bytes").and_then(|v| v.as_i64()),
            ) {
                let rows = row.get("rows").and_then(|v| v.as_i64());
                
                tables.push(TopTableBySize {
                    database: database.to_string(),
                    table: table.to_string(),
                    size_bytes,
                    rows,
                });
            }
        }
        
        Ok(tables)
    }

    /// Get top tables by access count
    /// Note: This requires audit logs to be enabled in StarRocks
    async fn get_top_tables_by_access(
        &self,
        cluster: &Cluster,
        limit: usize,
    ) -> ApiResult<Vec<TopTableByAccess>> {
        // Use AuditLogService to query audit logs
        let audit_service = AuditLogService::new(self.mysql_pool_manager.clone());
        
        // Query last 24 hours
        let audit_tables = audit_service
            .get_top_tables_by_access(cluster, 24, limit)
            .await?;
        
        // Convert AuditTopTableByAccess to TopTableByAccess
        let tables = audit_tables
            .into_iter()
            .map(|t| TopTableByAccess {
                database: t.database,
                table: t.table,
                access_count: t.access_count,
                last_access: t.last_access,
            })
            .collect();
        
        Ok(tables)
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
            mv_total, mv_active, mv_inactive
        );
        
        Ok((mv_total, mv_running, mv_failed, mv_success))
    }

    /// Get schema change statistics
    async fn get_schema_change_statistics(&self, client: &StarRocksClient) -> ApiResult<(i32, i32, i32, i32)> {
        let changes = client.get_schema_changes().await?;
        
        let running = changes.iter().filter(|c| c.state == "RUNNING").count() as i32;
        let pending = changes.iter().filter(|c| c.state == "PENDING").count() as i32;
        let finished = changes.iter().filter(|c| c.state == "FINISHED").count() as i32;
        let failed = changes.iter().filter(|c| c.state == "CANCELLED" || c.state == "FAILED").count() as i32;
        
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
            "#
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
}

