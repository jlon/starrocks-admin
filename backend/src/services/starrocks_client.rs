use crate::models::{
    Backend, Cluster, Database, Frontend, MaterializedView, Query, RuntimeInfo, SchemaChange, Table,
};
use crate::utils::{ApiError, ApiResult};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct StarRocksClient {
    pub http_client: Client,
    pub cluster: Cluster,
}

impl StarRocksClient {
    pub fn new(cluster: Cluster) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(cluster.connection_timeout as u64))
            .build()
            .unwrap_or_default();

        Self { http_client, cluster }
    }

    pub fn get_base_url(&self) -> String {
        let protocol = if self.cluster.enable_ssl { "https" } else { "http" };
        format!("{}://{}:{}", protocol, self.cluster.fe_host, self.cluster.fe_http_port)
    }

    // Get backends via HTTP API
    // Compatible with both shared-nothing and shared-data (compute-storage separation) architectures
    // For shared-data mode: tries /backends first, falls back to /compute_nodes if empty
    pub async fn get_backends(&self) -> ApiResult<Vec<Backend>> {
        // Try shared-nothing architecture first (traditional BE nodes)
        let url_be = format!("{}/api/show_proc?path=/backends", self.get_base_url());
        tracing::debug!("Fetching backends from: {}", url_be);

        let response_be = self
            .http_client
            .get(&url_be)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch backends: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response_be.status().is_success() {
            tracing::error!("Backends API returned error status: {}", response_be.status());
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response_be.status()
            )));
        }

        let data_be: Value = response_be.json().await.map_err(|e| {
            tracing::error!("Failed to parse backends response: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        // Log the raw response for debugging
        tracing::debug!("Raw backends response: {}", serde_json::to_string(&data_be).unwrap_or_default());

        // Try new format (direct array) first, then fall back to old format
        let backends_result = if let Ok(backends) = serde_json::from_value::<Vec<Backend>>(data_be.clone()) {
            Ok(backends)
        } else {
            // Fallback to old format
            match Self::parse_proc_result::<Backend>(&data_be) {
                Ok(backends) => {
                    tracing::info!("Retrieved {} backends using old format", backends.len());
                    Ok(backends)
                }
                Err(e) => {
                    tracing::error!("Failed to parse backends: {}. Raw response structure: {}", e, 
                        serde_json::to_string(&data_be).unwrap_or_else(|_| "failed to serialize".to_string()));
                    // Return error instead of empty vector to help diagnose the issue
                    Err(ApiError::internal_error(format!(
                        "Failed to parse backends response. Response format may have changed. Error: {}", e
                    )))
                }
            }
        };

        // If backends list is not empty, return it
        if let Ok(ref backends) = backends_result {
            if !backends.is_empty() {
                return backends_result;
            } else {
                tracing::warn!("Backends list is empty, trying compute nodes as fallback");
            }
        } else {
            // If parsing failed, log the error but continue to try compute nodes
            if let Err(ref e) = backends_result {
                tracing::warn!("Backends parsing failed: {}. Will try compute nodes as fallback.", e);
            }
        }

        // Try shared-data architecture (CN nodes for compute-storage separation)
        tracing::info!("No backends found, trying compute nodes for shared-data architecture");
        let url_cn = format!("{}/api/show_proc?path=/compute_nodes", self.get_base_url());
        tracing::debug!("Fetching compute nodes from: {}", url_cn);

        let response_cn = self
            .http_client
            .get(&url_cn)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await;

        match response_cn {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<Value>().await {
                    Ok(data_cn) => {
                        tracing::debug!("Raw compute nodes response: {}", serde_json::to_string(&data_cn).unwrap_or_default());
                        // Try to parse compute nodes as backends
                        if let Ok(compute_nodes) = serde_json::from_value::<Vec<Backend>>(data_cn.clone()) {
                            tracing::info!("Retrieved {} compute nodes (shared-data mode)", compute_nodes.len());
                            return Ok(compute_nodes);
                        } else if let Ok(compute_nodes) = Self::parse_proc_result::<Backend>(&data_cn) {
                            tracing::info!("Retrieved {} compute nodes using PROC format", compute_nodes.len());
                            return Ok(compute_nodes);
                        } else {
                            tracing::warn!("Failed to parse compute nodes response");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse compute nodes response: {}", e);
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("Compute nodes API returned error status: {}", resp.status());
            }
            Err(e) => {
                tracing::debug!("Compute nodes API request failed: {}", e);
            }
        }

        // If we got a successful parse but empty list, return it
        // If parsing failed, return the error to help diagnose the issue
        match backends_result {
            Ok(backends) => {
                tracing::warn!("No backends or compute nodes found. Returning empty list.");
                Ok(backends)
            }
            Err(e) => {
                tracing::error!("Failed to retrieve backends: {}. Please check StarRocks API response format.", e);
                // Return error so frontend can display it
                Err(e)
            }
        }
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

    // Get frontends via HTTP API
    pub async fn get_frontends(&self) -> ApiResult<Vec<Frontend>> {
        let url = format!("{}/api/show_proc?path=/frontends", self.get_base_url());
        tracing::debug!("Fetching frontends from: {}", url);

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch frontends: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            tracing::error!("Frontends API returned error status: {}", response.status());
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse frontends response: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        // Try new format (direct array) first, then fall back to old format
        if let Ok(frontends) = serde_json::from_value::<Vec<Frontend>>(data.clone()) {
            return Ok(frontends);
        }

        // Fallback to old format
        let frontends = Self::parse_proc_result::<Frontend>(&data)?;
        Ok(frontends)
    }

    // Get current queries
    pub async fn get_queries(&self) -> ApiResult<Vec<Query>> {
        let url = format!("{}/api/show_proc?path=/current_queries", self.get_base_url());

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| ApiError::cluster_connection_failed(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            tracing::warn!("Failed to fetch current_queries, HTTP status: {}", response.status());
            return Ok(Vec::new()); // Return empty list on HTTP errors
        }

        let data: Value = response.json().await.map_err(|e| {
            tracing::warn!("Failed to parse current_queries response: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        tracing::debug!("StarRocks /current_queries response: {}", data);

        // Handle empty array response (no running queries)
        if data.is_array() {
            match serde_json::from_value::<Vec<Query>>(data.clone()) {
                Ok(queries) => {
                    tracing::debug!("Parsed {} running queries from direct array", queries.len());
                    return Ok(queries);
                },
                Err(e) => tracing::debug!("Failed to parse array format: {}", e),
            }
        }

        // Handle empty object {} response
        if data.is_object() && data.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            tracing::debug!("StarRocks returned empty object, no running queries");
            return Ok(Vec::new());
        }

        // Try fallback format parsing, but tolerate errors
        match Self::parse_proc_result::<Query>(&data) {
            Ok(queries) => {
                tracing::debug!("Parsed {} running queries from PROC format", queries.len());
                Ok(queries)
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to parse PROC format for current_queries: {}. Returning empty list.",
                    e
                );
                Ok(Vec::new()) // Return empty list instead of failing
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

    // Parse PROC result format
    fn parse_proc_result<T: serde::de::DeserializeOwned>(data: &Value) -> ApiResult<Vec<T>> {
        // StarRocks PROC result format: {"columnNames": [...], "rows": [[...]]} or {"columns": [...], "rows": [...]}

        // Try to extract column names (support both "columnNames" and "columns" keys)
        let column_names = if let Some(cols) = data["columnNames"].as_array() {
            cols
        } else if let Some(cols) = data["columns"].as_array() {
            cols
        } else {
            // Log the actual data structure for debugging
            tracing::warn!(
                "Invalid PROC result format. Keys: {:?}, Data: {}",
                data.as_object().map(|o| o.keys().collect::<Vec<_>>()),
                data
            );
            return Err(ApiError::internal_error(format!(
                "Invalid PROC result format. Expected 'columnNames' or 'columns' field. Got: {}",
                data.as_object()
                    .map(|o| o.keys().map(|k| k.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default()
                    .join(", ")
            )));
        };

        let rows = data["rows"].as_array().ok_or_else(|| {
            ApiError::internal_error("Invalid PROC result format: missing 'rows' field")
        })?;

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

            let item: T = serde_json::from_value(Value::Object(obj))
                .map_err(|e| ApiError::internal_error(format!("Failed to parse item: {}", e)))?;

            results.push(item);
        }

        Ok(results)
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
        let url = format!("{}/api/show_proc?path=/dbs", self.get_base_url());
        tracing::debug!("Fetching databases from: {}", url);

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch databases: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            tracing::error!("Databases API returned error status: {}", response.status());
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse databases response: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        let databases = Self::parse_proc_result::<Database>(&data)?;
        tracing::debug!("Retrieved {} databases", databases.len());
        Ok(databases)
    }

    /// Get list of tables in a database
    #[allow(dead_code)]
    pub async fn get_tables(&self, database: &str) -> ApiResult<Vec<Table>> {
        let url = format!(
            "{}/api/show_proc?path=/dbs/{}/tables",
            self.get_base_url(),
            urlencoding::encode(database)
        );
        tracing::debug!("Fetching tables from database '{}': {}", database, url);

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch tables: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            tracing::error!("Tables API returned error status: {}", response.status());
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse tables response: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        let tables = Self::parse_proc_result::<Table>(&data)?;
        tracing::debug!("Retrieved {} tables from database '{}'", tables.len(), database);
        Ok(tables)
    }

    /// Get schema changes status
    #[allow(dead_code)]
    pub async fn get_schema_changes(&self) -> ApiResult<Vec<SchemaChange>> {
        let url = format!("{}/api/show_proc?path=/jobs", self.get_base_url());
        tracing::debug!("Fetching schema changes from: {}", url);

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&self.cluster.username, Some(&self.cluster.password_encrypted))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch schema changes: {}", e);
                ApiError::cluster_connection_failed(format!("Request failed: {}", e))
            })?;

        if !response.status().is_success() {
            tracing::error!("Schema changes API returned error status: {}", response.status());
            return Err(ApiError::cluster_connection_failed(format!(
                "HTTP status: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            tracing::error!("Failed to parse schema changes response: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to parse response: {}", e))
        })?;

        let changes = Self::parse_proc_result::<SchemaChange>(&data)?;
        tracing::debug!("Retrieved {} schema changes", changes.len());
        Ok(changes)
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
