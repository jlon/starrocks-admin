use crate::models::MaterializedView;
use crate::services::MySQLClient;
use crate::utils::{ApiError, ApiResult};

pub struct MaterializedViewService {
    mysql_client: MySQLClient,
}

impl MaterializedViewService {
    pub fn new(mysql_client: MySQLClient) -> Self {
        Self { mysql_client }
    }

    /// Get all materialized views (both async and sync)
    /// If database is None, fetches from all databases in the catalog
    /// Optimized version using information_schema system tables for better performance
    pub async fn list_materialized_views(
        &self,
        database: Option<&str>,
    ) -> ApiResult<Vec<MaterializedView>> {
        // Use information_schema for faster query and accurate row counts
        let sql = if let Some(db) = database {
            format!(
                "SELECT 
                    mv.MATERIALIZED_VIEW_ID as `id`,
                    mv.TABLE_NAME as `name`,
                    mv.TABLE_SCHEMA as database_name,
                    mv.REFRESH_TYPE as refresh_type,
                    mv.IS_ACTIVE as is_active,
                    mv.PARTITION_TYPE as partition_type,
                    mv.TASK_ID as task_id,
                    mv.TASK_NAME as task_name,
                    mv.LAST_REFRESH_START_TIME as last_refresh_start_time,
                    mv.LAST_REFRESH_FINISHED_TIME as last_refresh_finished_time,
                    mv.LAST_REFRESH_DURATION as last_refresh_duration,
                    mv.LAST_REFRESH_STATE as last_refresh_state,
                    COALESCE(t.TABLE_ROWS, 0) as `rows`,
                    mv.MATERIALIZED_VIEW_DEFINITION as `text`
                FROM information_schema.materialized_views mv
                LEFT JOIN information_schema.tables t 
                    ON mv.TABLE_SCHEMA = t.TABLE_SCHEMA AND mv.TABLE_NAME = t.TABLE_NAME
                WHERE mv.TABLE_SCHEMA = '{}'",
                db
            )
        } else {
            // Exclude system databases
            "SELECT 
                mv.MATERIALIZED_VIEW_ID as `id`,
                mv.TABLE_NAME as `name`,
                mv.TABLE_SCHEMA as database_name,
                mv.REFRESH_TYPE as refresh_type,
                mv.IS_ACTIVE as is_active,
                mv.PARTITION_TYPE as partition_type,
                mv.TASK_ID as task_id,
                mv.TASK_NAME as task_name,
                mv.LAST_REFRESH_START_TIME as last_refresh_start_time,
                mv.LAST_REFRESH_FINISHED_TIME as last_refresh_finished_time,
                mv.LAST_REFRESH_DURATION as last_refresh_duration,
                mv.LAST_REFRESH_STATE as last_refresh_state,
                COALESCE(t.TABLE_ROWS, 0) as `rows`,
                mv.MATERIALIZED_VIEW_DEFINITION as `text`
            FROM information_schema.materialized_views mv
            LEFT JOIN information_schema.tables t 
                ON mv.TABLE_SCHEMA = t.TABLE_SCHEMA AND mv.TABLE_NAME = t.TABLE_NAME
            WHERE mv.TABLE_SCHEMA NOT IN ('information_schema', '_statistics_')"
                .to_string()
        };

        tracing::info!("Querying materialized views using information_schema");
        let results = self.mysql_client.query(&sql).await?;
        let mvs = Self::parse_system_table_results(results)?;
        tracing::info!("Fetched {} materialized views", mvs.len());

        Ok(mvs)
    }

    /// Get a specific materialized view by name
    pub async fn get_materialized_view(&self, mv_name: &str) -> ApiResult<MaterializedView> {
        // Search across all databases
        let databases = self.get_all_databases().await?;

        for db in &databases {
            // Try async MVs
            if let Ok(mvs) = self.get_async_mvs_from_db(db).await
                && let Some(mv) = mvs.into_iter().find(|m| m.name == mv_name)
            {
                return Ok(mv);
            }

            // Try sync MVs
            if let Ok(mvs) = self.get_sync_mvs_from_db(db).await
                && let Some(mv) = mvs.into_iter().find(|m| m.name == mv_name)
            {
                return Ok(mv);
            }
        }

        Err(ApiError::not_found(format!("Materialized view '{}' not found", mv_name)))
    }

    /// Get DDL for a materialized view
    pub async fn get_materialized_view_ddl(&self, mv_name: &str) -> ApiResult<String> {
        // First, find which database the MV belongs to
        let mv = self.get_materialized_view(mv_name).await?;

        // For sync MVs (ROLLUP), the DDL is already stored in the 'text' field from SHOW MATERIALIZED VIEWS
        // ROLLUP MVs are indexes on tables, not standalone views, so SHOW CREATE MATERIALIZED VIEW doesn't work
        if mv.refresh_type == "ROLLUP" {
            if !mv.text.is_empty() {
                tracing::info!("Using DDL from MV text field for ROLLUP: {}", mv_name);
                return Ok(mv.text.clone());
            }
            // Fallback: try to construct DDL from SHOW ALTER MATERIALIZED VIEW
            // But for now, return the text field or error
            return Err(ApiError::not_found(format!(
                "DDL for sync MV (ROLLUP) '{}' not available. ROLLUP MVs are table indexes, not standalone views.",
                mv_name
            )));
        }

        // Create a session to use database context for async MVs
        let mut session = self.mysql_client.create_session().await?;
        session.use_database(&mv.database_name).await?;

        // For async MVs, use SHOW CREATE MATERIALIZED VIEW
        // Try different syntaxes
        let sql1 = format!("SHOW CREATE MATERIALIZED VIEW `{}`.`{}`", mv.database_name, mv_name);
        tracing::info!("Querying async MV DDL (attempt 1): {}", sql1);

        let (column_names, rows) = match session.execute(&sql1).await {
            Ok((cols, rows, _)) => (cols, rows),
            Err(_) => {
                // Fallback: try without database prefix (already in database context)
                let sql2 = format!("SHOW CREATE MATERIALIZED VIEW `{}`", mv_name);
                tracing::info!("Querying async MV DDL (attempt 2): {}", sql2);
                let (cols, rows, _) = session.execute(&sql2).await?;
                (cols, rows)
            },
        };

        // Convert rows to JSON format for consistent processing
        let mut results = Vec::new();
        for row in rows {
            let mut obj = serde_json::Map::new();
            for (i, col_name) in column_names.iter().enumerate() {
                if let Some(value) = row.get(i) {
                    obj.insert(col_name.clone(), serde_json::Value::String(value.clone()));
                }
            }
            results.push(serde_json::Value::Object(obj));
        }

        // Extract DDL from result
        // StarRocks returns column name as "Create Materialized View" or "Create View"
        if let Some(row) = results.first() {
            // Try different possible column names
            if let Some(ddl_val) = row.get("Create Materialized View")
                && let Some(ddl) = ddl_val.as_str()
            {
                return Ok(ddl.to_string());
            }
            if let Some(ddl_val) = row.get("Create View")
                && let Some(ddl) = ddl_val.as_str()
            {
                return Ok(ddl.to_string());
            }
            // If column name doesn't match, try to get the first string value from the row
            if let Some(obj) = row.as_object() {
                for (_key, value) in obj {
                    if let Some(ddl) = value.as_str() {
                        return Ok(ddl.to_string());
                    }
                }
            }
        }

        Err(ApiError::not_found(format!("DDL for materialized view '{}' not found", mv_name)))
    }

    /// Create a materialized view
    pub async fn create_materialized_view(&self, sql: &str) -> ApiResult<()> {
        tracing::info!("Creating materialized view with SQL: {}", sql);
        self.mysql_client.execute(sql).await?;
        Ok(())
    }

    /// Drop a materialized view
    pub async fn drop_materialized_view(&self, mv_name: &str, if_exists: bool) -> ApiResult<()> {
        // First, find which database the MV belongs to (if not using IF EXISTS, we need to know)
        // For IF EXISTS, we still try to find it, but don't fail if not found
        let database = if if_exists {
            match self.get_materialized_view(mv_name).await {
                Ok(mv) => Some(mv.database_name),
                Err(_) => None,
            }
        } else {
            Some(self.get_materialized_view(mv_name).await?.database_name)
        };

        let sql = if let Some(db) = database {
            if if_exists {
                format!("DROP MATERIALIZED VIEW IF EXISTS `{}`.`{}`", db, mv_name)
            } else {
                format!("DROP MATERIALIZED VIEW `{}`.`{}`", db, mv_name)
            }
        } else {
            // Fallback for IF EXISTS when MV not found
            format!("DROP MATERIALIZED VIEW IF EXISTS `{}`", mv_name)
        };

        tracing::info!("Dropping materialized view: {}", sql);
        self.mysql_client.execute(&sql).await?;
        Ok(())
    }

    /// Refresh a materialized view
    pub async fn refresh_materialized_view(
        &self,
        mv_name: &str,
        partition_start: Option<&str>,
        partition_end: Option<&str>,
        force: bool,
        mode: &str,
    ) -> ApiResult<()> {
        // First, find which database the MV belongs to
        let mv = self.get_materialized_view(mv_name).await?;

        // Build SQL with database name
        let mut sql = format!("REFRESH MATERIALIZED VIEW `{}`.`{}`", mv.database_name, mv_name);

        if let (Some(start), Some(end)) = (partition_start, partition_end) {
            sql.push_str(&format!(" PARTITION START ('{}') END ('{}')", start, end));
        }

        if force {
            sql.push_str(" FORCE");
        }

        sql.push_str(&format!(" WITH {} MODE", mode));

        tracing::info!("Refreshing materialized view: {}", sql);
        self.mysql_client.execute(&sql).await?;
        Ok(())
    }

    /// Cancel refresh of a materialized view
    pub async fn cancel_refresh_materialized_view(
        &self,
        mv_name: &str,
        force: bool,
    ) -> ApiResult<()> {
        // First, find which database the MV belongs to
        let mv = self.get_materialized_view(mv_name).await?;

        let sql = if force {
            format!("CANCEL REFRESH MATERIALIZED VIEW `{}`.`{}` FORCE", mv.database_name, mv_name)
        } else {
            format!("CANCEL REFRESH MATERIALIZED VIEW `{}`.`{}`", mv.database_name, mv_name)
        };
        tracing::info!("Cancelling refresh for materialized view: {}", sql);
        self.mysql_client.execute(&sql).await?;
        Ok(())
    }

    /// Alter a materialized view
    pub async fn alter_materialized_view(
        &self,
        mv_name: &str,
        alter_clause: &str,
    ) -> ApiResult<()> {
        // First, find which database the MV belongs to
        let mv = self.get_materialized_view(mv_name).await?;

        let sql = format!(
            "ALTER MATERIALIZED VIEW `{}`.`{}` {}",
            mv.database_name, mv_name, alter_clause
        );
        tracing::info!("Altering materialized view: {}", sql);
        self.mysql_client.execute(&sql).await?;
        Ok(())
    }

    // ========== Private Helper Methods ==========

    /// Get all databases (excluding system databases)
    async fn get_all_databases(&self) -> ApiResult<Vec<String>> {
        let sql = "SHOW DATABASES";
        tracing::debug!("Querying databases: {}", sql);

        let results = self.mysql_client.query(sql).await?;
        let mut databases = Vec::new();

        for row in results {
            if let Some(db_name) = row.get("Database").and_then(|v| v.as_str()) {
                // Skip system databases
                if db_name != "information_schema" && db_name != "_statistics_" {
                    databases.push(db_name.to_string());
                }
            }
        }

        tracing::debug!("Found {} databases", databases.len());
        Ok(databases)
    }

    /// Get async materialized views from a specific database
    async fn get_async_mvs_from_db(&self, database: &str) -> ApiResult<Vec<MaterializedView>> {
        let sql = format!("SHOW MATERIALIZED VIEWS FROM `{}`", database);
        tracing::debug!("Querying async MVs: {}", sql);

        let results = self.mysql_client.query(&sql).await?;
        Self::parse_async_mv_results(results, database)
    }

    /// Get sync materialized views (ROLLUP) from a specific database
    async fn get_sync_mvs_from_db(&self, database: &str) -> ApiResult<Vec<MaterializedView>> {
        let sql = format!("SHOW ALTER MATERIALIZED VIEW FROM `{}`", database);
        tracing::debug!("Querying sync MVs: {}", sql);

        let results = self.mysql_client.query(&sql).await?;
        Self::parse_sync_mv_results(results, database)
    }

    /// Parse SHOW MATERIALIZED VIEWS result
    fn parse_async_mv_results(
        results: Vec<serde_json::Value>,
        database: &str,
    ) -> ApiResult<Vec<MaterializedView>> {
        let mut mvs = Vec::new();

        for row in results {
            let mv = MaterializedView {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: row
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                database_name: row
                    .get("database_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(database)
                    .to_string(),
                refresh_type: row
                    .get("refresh_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string(),
                is_active: row
                    .get("is_active")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "true" || s == "1")
                    .unwrap_or(false),
                partition_type: row
                    .get("partition_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                task_id: row
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                task_name: row
                    .get("task_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_start_time: row
                    .get("last_refresh_start_time")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_finished_time: row
                    .get("last_refresh_finished_time")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_duration: row
                    .get("last_refresh_duration")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_state: row
                    .get("last_refresh_state")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                rows: row.get("rows").and_then(|v| v.as_i64()),
                text: row
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            };

            mvs.push(mv);
        }

        Ok(mvs)
    }

    /// Parse SHOW ALTER MATERIALIZED VIEW result (sync MVs/ROLLUP)
    fn parse_sync_mv_results(
        results: Vec<serde_json::Value>,
        database: &str,
    ) -> ApiResult<Vec<MaterializedView>> {
        let mut mvs = Vec::new();

        for row in results {
            // Only include FINISHED sync MVs
            let state = row.get("State").and_then(|v| v.as_str()).unwrap_or("");

            if state != "FINISHED" {
                continue;
            }

            let mv_name = row
                .get("RollupIndexName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let table_name = row
                .get("TableName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let mv = MaterializedView {
                id: format!("sync_{}", mv_name),
                name: mv_name.clone(),
                database_name: database.to_string(),
                refresh_type: "ROLLUP".to_string(),
                is_active: true,
                partition_type: None,
                task_id: None,
                task_name: None,
                last_refresh_start_time: row
                    .get("CreateTime")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_finished_time: row
                    .get("FinishedTime")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_duration: None,
                last_refresh_state: Some("SUCCESS".to_string()),
                rows: None,
                text: format!("-- Sync materialized view on table: {}", table_name),
            };

            mvs.push(mv);
        }

        Ok(mvs)
    }

    /// Parse results from information_schema system tables
    fn parse_system_table_results(
        results: Vec<serde_json::Value>,
    ) -> ApiResult<Vec<MaterializedView>> {
        let mut mvs = Vec::new();

        for row in results {
            let mv = MaterializedView {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: row
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                database_name: row
                    .get("database_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                refresh_type: row
                    .get("refresh_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string(),
                is_active: row
                    .get("is_active")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "true" || s == "1")
                    .unwrap_or(false),
                partition_type: row
                    .get("partition_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                task_id: row
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                task_name: row
                    .get("task_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_start_time: row
                    .get("last_refresh_start_time")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_finished_time: row
                    .get("last_refresh_finished_time")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_duration: row
                    .get("last_refresh_duration")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_refresh_state: row
                    .get("last_refresh_state")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                rows: row.get("rows").and_then(|v| {
                    // Try as i64 first, then as string
                    v.as_i64()
                        .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                }),
                text: row
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            };

            mvs.push(mv);
        }

        Ok(mvs)
    }
}
