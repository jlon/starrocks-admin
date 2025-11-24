use crate::models::{
    Backend, Cluster, Database, Frontend, MaterializedView, Query, RuntimeInfo, SchemaChange, Table,
};
use crate::services::{mysql_client::MySQLClient, mysql_pool_manager::MySQLPoolManager};
use crate::utils::{ApiError, ApiResult};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

pub struct StarRocksClient {
    pub http_client: Client,
    pub cluster: Cluster,
    mysql_pool_manager: Arc<MySQLPoolManager>,
}

impl StarRocksClient {
    pub fn new(cluster: Cluster, mysql_pool_manager: Arc<MySQLPoolManager>) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(cluster.connection_timeout as u64))
            .build()
            .unwrap_or_default();

        Self { http_client, cluster, mysql_pool_manager }
    }

    pub fn get_base_url(&self) -> String {
        let protocol = if self.cluster.enable_ssl { "https" } else { "http" };
        format!("{}://{}:{}", protocol, self.cluster.fe_host, self.cluster.fe_http_port)
    }

    async fn mysql_client(&self) -> ApiResult<MySQLClient> {
        let pool = self.mysql_pool_manager.get_pool(&self.cluster).await?;
        Ok(MySQLClient::from_pool(pool))
    }

    fn normalize_proc_path(path: &str) -> String {
        if path.is_empty() {
            "/".to_string()
        } else if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        }
    }

    fn escape_proc_path(path: &str) -> String {
        path.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn build_show_proc_sql(path: &str) -> String {
        let normalized = Self::normalize_proc_path(path);
        let escaped = Self::escape_proc_path(&normalized);
        format!("SHOW PROC \"{}\"", escaped)
    }

    pub async fn show_proc_raw(&self, path: &str) -> ApiResult<Vec<Value>> {
        let sql = Self::build_show_proc_sql(path);
        let mysql_client = self.mysql_client().await?;
        mysql_client.query(&sql).await
    }

    async fn show_proc_entities<T>(&self, path: &str) -> ApiResult<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let rows = self.show_proc_raw(path).await?;
        let mut entities = Vec::with_capacity(rows.len());

        for row in rows {
            match serde_json::from_value::<T>(row) {
                Ok(value) => entities.push(value),
                Err(e) => {
                    tracing::warn!("Failed to deserialize SHOW PROC '{}' row: {}", path, e);
                },
            }
        }

        Ok(entities)
    }

    pub async fn get_backends(&self) -> ApiResult<Vec<Backend>> {
        tracing::debug!("Fetching backends via MySQL SHOW PROC");

        match self.show_proc_entities::<Backend>("/backends").await {
            Ok(backends) if !backends.is_empty() => {
                tracing::debug!("Retrieved {} backend entries", backends.len());
                return Ok(backends);
            },
            Ok(_) => {
                tracing::warn!(
                    "SHOW PROC /backends returned empty result, falling back to /compute_nodes"
                );
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to retrieve /backends via MySQL interface: {}. Falling back to /compute_nodes",
                    e
                );
            },
        }

        let compute_nodes = self.show_proc_entities::<Backend>("/compute_nodes").await?;
        if compute_nodes.is_empty() {
            tracing::warn!("No backends or compute nodes found via SHOW PROC");
        } else {
            tracing::info!(
                "Retrieved {} compute nodes via SHOW PROC /compute_nodes",
                compute_nodes.len()
            );
        }
        Ok(compute_nodes)
    }

    // Execute SQL command via HTTP API
    pub async fn execute_sql(&self, sql: &str) -> ApiResult<()> {
        let url = format!("{}/api/query", self.get_base_url());
        tracing::debug!("Executing SQL: {}", sql);

        let body = serde_json::json!({
            "query": sql
        });

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to execute SQL: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("SQL execution failed with status {}: {}", status, error_text);
            return Err(ApiError::cluster_connection_failed(format!(
                "SQL execution failed: {}",
                error_text
            )));
        }

        tracing::info!("SQL executed successfully: {}", sql);
        Ok(())
    }

    // Drop backend node
    pub async fn drop_backend(&self, host: &str, heartbeat_port: &str) -> ApiResult<()> {
        let sql = format!("ALTER SYSTEM DROP backend \"{}:{}\"", host, heartbeat_port);
        tracing::info!("Dropping backend: {}:{}", host, heartbeat_port);
        self.execute_sql(&sql).await
    }

    pub async fn get_frontends(&self) -> ApiResult<Vec<Frontend>> {
        tracing::debug!("Fetching frontends via MySQL SHOW PROC");
        self.show_proc_entities::<Frontend>("/frontends").await
    }

    // Get current queries
    pub async fn get_queries(&self) -> ApiResult<Vec<Query>> {
        match self.show_proc_entities::<Query>("/current_queries").await {
            Ok(queries) => Ok(queries),
            Err(e) => {
                tracing::warn!(
                    "Failed to retrieve /current_queries via SHOW PROC: {}. Returning empty list.",
                    e
                );
                Ok(Vec::new())
            },
        }
    }

    // Get runtime info
    pub async fn get_runtime_info(&self) -> ApiResult<RuntimeInfo> {
        let url = format!("{}/api/show_runtime_info", self.get_base_url());

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| ApiError::cluster_connection_failed(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let runtime_info: RuntimeInfo = response.json().await.map_err(|e| {
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        Ok(runtime_info)
    }

    // Get metrics in Prometheus format
    pub async fn get_metrics(&self) -> ApiResult<String> {
        let url = format!("{}/metrics", self.get_base_url());

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| ApiError::cluster_connection_failed(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let metrics_text = response.text().await.map_err(|e| {
            ApiError::cluster_connection_failed(format!("Failed to read response: {}", e))
        })?;

        Ok(metrics_text)
    }

    // Parse Prometheus metrics format
    pub fn parse_prometheus_metrics(
        &self,
        metrics_text: &str,
    ) -> ApiResult<std::collections::HashMap<String, f64>> {
        let mut metrics = std::collections::HashMap::new();

        for line in metrics_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse format: metric_name{labels} value
            if let Some((name_part, value_str)) = line.rsplit_once(' ')
                && let Ok(value) = value_str.parse::<f64>()
            {
                // Extract metric name (before '{' or the whole name_part)
                let metric_name =
                    if let Some(pos) = name_part.find('{') { &name_part[..pos] } else { name_part };

                metrics.insert(metric_name.to_string(), value);
            }
        }

        Ok(metrics)
    }

    // Get materialized views list (both async and sync/ROLLUP)
    // If database is None, fetches MVs from all databases in the catalog
    #[allow(dead_code)]
    pub async fn get_materialized_views(
        &self,
        database: Option<&str>,
    ) -> ApiResult<Vec<MaterializedView>> {
        let mut all_mvs = Vec::new();

        // If database specified, only query that database
        if let Some(db) = database {
            // Get async MVs
            let async_mvs = self.get_async_materialized_views(Some(db)).await?;
            all_mvs.extend(async_mvs);

            // Get sync MVs
            let sync_mvs = self
                .get_sync_materialized_views(Some(db))
                .await
                .unwrap_or_default();
            all_mvs.extend(sync_mvs);
        } else {
            // Get all databases first, then query each database
            // Use default catalog (None means use cluster's default catalog)
            let databases = self.get_all_databases(None).await?;

            for db in &databases {
                // Get async MVs from this database
                if let Ok(async_mvs) = self.get_async_materialized_views(Some(db)).await {
                    all_mvs.extend(async_mvs);
                }

                // Get sync MVs from this database
                if let Ok(sync_mvs) = self.get_sync_materialized_views(Some(db)).await {
                    all_mvs.extend(sync_mvs);
                }
            }
        }

        tracing::debug!("Retrieved {} total materialized views (async + sync)", all_mvs.len());
        Ok(all_mvs)
    }

    // Get all databases in the specified catalog
    pub async fn get_all_databases(&self, catalog: Option<&str>) -> ApiResult<Vec<String>> {
        let sql = "SHOW DATABASES";
        tracing::debug!("Fetching databases with SQL: {}", sql);

        let catalog_name = catalog.unwrap_or(&self.cluster.catalog);
        let url = format!("{}/api/v1/catalogs/{}/sql", self.get_base_url(), catalog_name);

        let body = serde_json::json!({
            "query": sql
        });

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch databases: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            tracing::warn!("Failed to fetch databases: {}", response.status());
            return Ok(Vec::new());
        }

        let data: Value = response.json().await.map_err(|e| {
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        // Parse SHOW DATABASES result - try multiple formats
        let result_data = data.get("data").unwrap_or(&data);
        let mut databases = Vec::new();

        // Try PROC format first: {"columnNames": [...], "rows": [[...]]}
        if let Some(rows) = result_data.get("rows").and_then(|v| v.as_array()) {
            for row in rows {
                if let Some(row_array) = row.as_array() {
                    // Find Database column - could be first column or by name
                    if let Some(db_name_value) = row_array.first() {
                        let db_name = if let Some(name_str) = db_name_value.as_str() {
                            name_str.trim().to_string()
                        } else {
                            continue;
                        };

                        // Skip system databases and empty names
                        if !db_name.is_empty()
                            && db_name != "information_schema"
                            && db_name != "_statistics_"
                        {
                            databases.push(db_name);
                        }
                    }
                }
            }
        }

        tracing::debug!(
            "Found {} databases in catalog {} (or default)",
            databases.len(),
            catalog_name
        );
        Ok(databases)
    }

    // Get async materialized views only
    #[allow(dead_code)]
    async fn get_async_materialized_views(
        &self,
        database: Option<&str>,
    ) -> ApiResult<Vec<MaterializedView>> {
        // Build SQL: SHOW MATERIALIZED VIEWS [FROM database]
        let sql = if let Some(db) = database {
            format!("SHOW MATERIALIZED VIEWS FROM `{}`", db)
        } else {
            "SHOW MATERIALIZED VIEWS".to_string()
        };

        tracing::debug!("Fetching async materialized views with SQL: {}", sql);

        // Use /api/v1/catalogs/{catalog}/sql endpoint to execute SQL
        let catalog = &self.cluster.catalog;
        let url = format!("{}/api/v1/catalogs/{}/sql", self.get_base_url(), catalog);

        let body = serde_json::json!({
            "query": sql
        });

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch async materialized views: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!(
                "Failed to fetch async materialized views with status {}: {}",
                status,
                error_text
            );
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                error_text
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse async materialized views response: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        // Parse result using the same logic as other PROC results
        let mvs = Self::parse_mv_result(&data)?;
        tracing::debug!("Retrieved {} async materialized views", mvs.len());
        Ok(mvs)
    }

    // Get sync materialized views (ROLLUP) from SHOW ALTER MATERIALIZED VIEW
    #[allow(dead_code)]
    async fn get_sync_materialized_views(
        &self,
        database: Option<&str>,
    ) -> ApiResult<Vec<MaterializedView>> {
        let sql = if let Some(db) = database {
            format!("SHOW ALTER MATERIALIZED VIEW FROM `{}`", db)
        } else {
            "SHOW ALTER MATERIALIZED VIEW".to_string()
        };

        tracing::debug!("Fetching sync materialized views with SQL: {}", sql);

        let catalog = &self.cluster.catalog;
        let url = format!("{}/api/v1/catalogs/{}/sql", self.get_base_url(), catalog);

        let body = serde_json::json!({
            "query": sql
        });

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("Failed to fetch sync materialized views: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            tracing::warn!("Sync MV query returned non-success status: {}", response.status());
            return Ok(Vec::new()); // Return empty if sync MVs not supported
        }

        let data: Value = response.json().await.map_err(|e| {
            tracing::warn!("Failed to parse sync materialized views response: {}", e);
            ApiError::internal_error(format!("Failed to parse response: {}", e))
        })?;

        // Parse SHOW ALTER MATERIALIZED VIEW result
        let sync_mvs = Self::parse_sync_mv_result(&data, database)?;
        tracing::debug!("Retrieved {} sync materialized views", sync_mvs.len());
        Ok(sync_mvs)
    }

    // Get single materialized view details
    #[allow(dead_code)]
    pub async fn get_materialized_view(&self, mv_name: &str) -> ApiResult<MaterializedView> {
        let sql = format!("SHOW MATERIALIZED VIEWS WHERE NAME = '{}'", mv_name);
        tracing::debug!("Fetching materialized view details with SQL: {}", sql);

        let catalog = &self.cluster.catalog;
        let url = format!("{}/api/v1/catalogs/{}/sql", self.get_base_url(), catalog);

        let body = serde_json::json!({
            "query": sql
        });

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::cluster_connection_failed(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        let mvs = Self::parse_mv_result(&data)?;
        mvs.into_iter().next().ok_or_else(|| {
            ApiError::not_found(format!("Materialized view '{}' not found", mv_name))
        })
    }

    // Get materialized view DDL
    #[allow(dead_code)]
    pub async fn get_materialized_view_ddl(&self, mv_name: &str) -> ApiResult<String> {
        let sql = format!("SHOW CREATE MATERIALIZED VIEW `{}`", mv_name);
        tracing::debug!("Fetching materialized view DDL with SQL: {}", sql);

        let catalog = &self.cluster.catalog;
        let url = format!("{}/api/v1/catalogs/{}/sql", self.get_base_url(), catalog);

        let body = serde_json::json!({
            "query": sql
        });

        let response = self
            .http_client
            .post(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::cluster_connection_failed(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        // Extract DDL from result
        // SHOW CREATE MATERIALIZED VIEW returns: [[mv_name, create_statement]]
        if let Some(rows) = data["data"].as_array()
            && let Some(row) = rows.first()
            && let Some(row_array) = row.as_array()
            && let Some(ddl) = row_array.get(1)
            && let Some(ddl_str) = ddl.as_str()
        {
            return Ok(ddl_str.to_string());
        }

        Err(ApiError::internal_error("Failed to extract DDL from response"))
    }

    // Parse materialized view result format
    #[allow(dead_code)]
    fn parse_mv_result(data: &Value) -> ApiResult<Vec<MaterializedView>> {
        // Check if data has "data" field (new format) or use root (old format)
        let result_data = data.get("data").unwrap_or(data);

        // Try to parse as array of objects directly
        if let Ok(mvs) = serde_json::from_value::<Vec<MaterializedView>>(result_data.clone()) {
            return Ok(mvs);
        }

        // Try PROC result format: {"columnNames": [...], "rows": [[...]]}
        if let Some(column_names) = result_data.get("columnNames").and_then(|v| v.as_array())
            && let Some(rows) = result_data.get("rows").and_then(|v| v.as_array())
        {
            let mut results = Vec::new();

            for row in rows {
                let row_array = row
                    .as_array()
                    .ok_or_else(|| ApiError::internal_error("Invalid row format"))?;

                // Create a JSON object from column names and row values
                let mut obj = serde_json::Map::new();
                for (i, col_name) in column_names.iter().enumerate() {
                    if let Some(col_name_str) = col_name.as_str()
                        && let Some(value) = row_array.get(i)
                    {
                        obj.insert(col_name_str.to_string(), value.clone());
                    }
                }

                let mv: MaterializedView = serde_json::from_value(Value::Object(obj))
                    .map_err(|e| ApiError::internal_error(format!("Failed to parse MV: {}", e)))?;

                results.push(mv);
            }

            return Ok(results);
        }

        Err(ApiError::internal_error("Unsupported materialized view result format"))
    }

    // Parse SHOW ALTER MATERIALIZED VIEW result to MaterializedView format
    // Returns FINISHED sync MVs only
    #[allow(dead_code)]
    fn parse_sync_mv_result(
        data: &Value,
        database: Option<&str>,
    ) -> ApiResult<Vec<MaterializedView>> {
        let result_data = data.get("data").unwrap_or(data);

        // Try PROC result format: {"columnNames": [...], "rows": [[...]]}
        if let Some(column_names) = result_data.get("columnNames").and_then(|v| v.as_array())
            && let Some(rows) = result_data.get("rows").and_then(|v| v.as_array())
        {
            let mut results = Vec::new();

            // Find column indices
            let mut table_name_idx = None;
            let mut rollup_name_idx = None;
            let mut state_idx = None;
            let mut create_time_idx = None;
            let mut finished_time_idx = None;

            for (i, col_name) in column_names.iter().enumerate() {
                if let Some(col_str) = col_name.as_str() {
                    match col_str {
                        "TableName" => table_name_idx = Some(i),
                        "RollupIndexName" => rollup_name_idx = Some(i),
                        "State" => state_idx = Some(i),
                        "CreateTime" => create_time_idx = Some(i),
                        "FinishedTime" => finished_time_idx = Some(i),
                        _ => {},
                    }
                }
            }

            for row in rows {
                if let Some(row_array) = row.as_array() {
                    // Only include FINISHED sync MVs
                    if let Some(state_idx) = state_idx
                        && let Some(state) = row_array.get(state_idx).and_then(|v| v.as_str())
                        && state != "FINISHED"
                    {
                        continue; // Skip non-finished MVs
                    }

                    // Extract values
                    let mv_name = rollup_name_idx
                        .and_then(|idx| row_array.get(idx))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let table_name = table_name_idx
                        .and_then(|idx| row_array.get(idx))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let create_time = create_time_idx
                        .and_then(|idx| row_array.get(idx))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let finished_time = finished_time_idx
                        .and_then(|idx| row_array.get(idx))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    // Build MaterializedView struct for sync MV
                    let mv = MaterializedView {
                        id: format!("sync_{}", mv_name), // Generate ID for sync MVs
                        name: mv_name.clone(),
                        database_name: database.unwrap_or("").to_string(),
                        refresh_type: "ROLLUP".to_string(), // Sync MVs are ROLLUP type
                        is_active: true,                    // FINISHED state means active
                        partition_type: None, // Sync MVs don't have partition info from SHOW ALTER
                        task_id: None,
                        task_name: None,
                        last_refresh_start_time: create_time,
                        last_refresh_finished_time: finished_time,
                        last_refresh_duration: None,
                        last_refresh_state: Some("SUCCESS".to_string()),
                        rows: None,
                        text: format!("-- Sync materialized view on table: {}", table_name),
                    };

                    results.push(mv);
                }
            }

            return Ok(results);
        }

        Ok(Vec::new()) // Return empty if can't parse
    }

    // ========================================
    // New methods for Cluster Overview
    // ========================================

    /// Get list of databases
    #[allow(dead_code)]
    pub async fn get_databases(&self) -> ApiResult<Vec<Database>> {
        tracing::debug!("Fetching databases via SHOW PROC /dbs");
        let databases = self.show_proc_entities::<Database>("/dbs").await?;
        tracing::debug!("Retrieved {} databases", databases.len());
        Ok(databases)
    }

    /// Get list of tables in a database
    /// Note: Path /dbs/{db_name}/tables doesn't exist. Two options:
    /// 1. Use SHOW TABLES FROM {db} (simple, fast)
    /// 2. Use /dbs/{db_id} (requires finding db_id first, but returns more details)
    #[allow(dead_code)]
    pub async fn get_tables(&self, database: &str) -> ApiResult<Vec<Table>> {
        tracing::debug!("Fetching tables from database '{}'", database);

        // Try method 1: Use SHOW TABLES (simpler and faster)
        let mysql_client = self.mysql_client().await?;
        let sql = format!("SHOW TABLES FROM `{}`", database);

        match mysql_client.query(&sql).await {
            Ok(rows) => {
                let mut tables = Vec::new();
                for row in rows {
                    if let serde_json::Value::Object(obj) = row {
                        // SHOW TABLES returns a single column with table name
                        // Column name varies: "Tables_in_{db}" or first key
                        let table_name = obj
                            .values()
                            .next()
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();

                        if !table_name.is_empty() {
                            tables.push(Table {
                                table_name: table_name.clone(),
                                table_type: "BASE TABLE".to_string(), // Default type
                                engine: None,
                            });
                        }
                    }
                }
                tracing::debug!(
                    "Retrieved {} tables from database '{}' via SHOW TABLES",
                    tables.len(),
                    database
                );
                return Ok(tables);
            },
            Err(e) => {
                tracing::warn!(
                    "SHOW TABLES failed for database '{}': {}. Trying PROC method.",
                    database,
                    e
                );
            },
        }

        // Fallback method 2: Use /dbs/{db_id} (requires finding db_id first)
        // First, get all databases to find the db_id
        let databases = self.show_proc_entities::<Database>("/dbs").await?;
        let db_id = databases
            .iter()
            .find(|db| db.database == database)
            .and_then(|db| db.db_id.as_ref())
            .ok_or_else(|| ApiError::not_found(format!("Database '{}' not found", database)))?;

        // Now get tables using /dbs/{db_id}
        let path = format!("/dbs/{}", db_id);
        let table_rows = self.show_proc_raw(&path).await?;

        let mut tables = Vec::new();
        for row in table_rows {
            if let serde_json::Value::Object(obj) = row {
                let table_name = obj
                    .get("TableName")
                    .or_else(|| obj.get("table_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                if !table_name.is_empty() {
                    let table_type = obj
                        .get("Type")
                        .or_else(|| obj.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("BASE TABLE")
                        .to_string();

                    tables.push(Table { table_name, table_type, engine: None });
                }
            }
        }

        tracing::debug!(
            "Retrieved {} tables from database '{}' via PROC /dbs/{}",
            tables.len(),
            database,
            db_id
        );
        Ok(tables)
    }

    /// Get schema changes status
    /// Note: SHOW PROC '/jobs' returns database list. To get schema change jobs:
    /// 1. Get all databases from /jobs (returns DbId, DbName)
    /// 2. For each database, query /jobs/{db_id}/schema_change
    #[allow(dead_code)]
    pub async fn get_schema_changes(&self) -> ApiResult<Vec<SchemaChange>> {
        tracing::debug!("Fetching schema change jobs from all databases");

        // Step 1: Get all databases from /jobs (returns DbId, DbName)
        let databases = self.show_proc_raw("/jobs").await?;

        let mut all_changes = Vec::new();

        // Step 2: For each database, query schema change jobs
        for db_row in databases {
            if let serde_json::Value::Object(db_obj) = db_row {
                // Extract DbId
                let db_id = db_obj
                    .get("DbId")
                    .or_else(|| db_obj.get("db_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                if db_id.is_empty() {
                    continue;
                }

                // Query schema change jobs for this database
                let path = format!("/jobs/{}/schema_change", db_id);
                match self.show_proc_entities::<SchemaChange>(&path).await {
                    Ok(changes) => {
                        tracing::debug!(
                            "Found {} schema change jobs in database {}",
                            changes.len(),
                            db_id
                        );
                        all_changes.extend(changes);
                    },
                    Err(e) => {
                        // Ignore errors for databases with no schema change jobs
                        tracing::debug!(
                            "No schema change jobs in database {} or error: {}",
                            db_id,
                            e
                        );
                    },
                }
            }
        }

        tracing::debug!("Retrieved {} total schema change jobs", all_changes.len());
        Ok(all_changes)
    }

    /// Get active users from current queries
    #[allow(dead_code)]
    pub async fn get_active_users(&self) -> ApiResult<Vec<String>> {
        let queries = self.get_queries().await?;

        // Extract unique users
        let mut users: Vec<String> = queries
            .iter()
            .map(|q| q.user.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        users.sort();
        tracing::debug!("Retrieved {} active users", users.len());
        Ok(users)
    }

    /// Get database count
    #[allow(dead_code)]
    pub async fn get_database_count(&self) -> ApiResult<usize> {
        let databases = self.get_databases().await?;
        Ok(databases.len())
    }

    /// Get total table count across all databases
    #[allow(dead_code)]
    pub async fn get_total_table_count(&self) -> ApiResult<usize> {
        let databases = self.get_databases().await?;
        let mut total_tables = 0;

        for db in databases {
            match self.get_tables(&db.database).await {
                Ok(tables) => total_tables += tables.len(),
                Err(e) => {
                    tracing::warn!("Failed to get tables for database '{}': {}", db.database, e);
                    // Continue with other databases
                },
            }
        }

        tracing::debug!("Total table count: {}", total_tables);
        Ok(total_tables)
    }
}
