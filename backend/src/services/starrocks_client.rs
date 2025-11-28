use crate::models::{Backend, Cluster, Frontend, Query, RuntimeInfo};
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
            .unwrap_or_else(|e| {
                // HTTP client build failure is rare and usually indicates system resource issues
                tracing::error!(
                    "Failed to build HTTP client for cluster {}: {}. This is a critical error.",
                    cluster.name,
                    e
                );
                // Use default client as fallback but log the issue
                tracing::warn!("Using default HTTP client configuration as fallback");
                Client::default()
            });

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
        // Automatically switch query strategy based on deployment mode
        if self.cluster.is_shared_data() {
            tracing::info!(
                "Cluster {} is in shared-data mode, fetching compute nodes",
                self.cluster.name
            );
            return self.get_compute_nodes().await;
        }

        tracing::debug!(
            "Cluster {} is in shared-nothing mode, fetching backends",
            self.cluster.name
        );
        self.show_proc_entities::<Backend>("/backends").await
    }

    // Get compute nodes for shared-data architecture
    async fn get_compute_nodes(&self) -> ApiResult<Vec<Backend>> {
        let compute_nodes = self.show_proc_entities::<Backend>("/compute_nodes").await?;
        tracing::info!("Retrieved {} compute nodes (shared-data mode)", compute_nodes.len());
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

    // Drop backend node (BE for shared-nothing, CN for shared-data)
    pub async fn drop_backend(&self, host: &str, heartbeat_port: &str) -> ApiResult<()> {
        let sql = if self.cluster.is_shared_data() {
            // Shared-data mode: drop compute node
            format!("ALTER SYSTEM DROP COMPUTE NODE \"{}:{}\"", host, heartbeat_port)
        } else {
            // Shared-nothing mode: drop backend
            format!("ALTER SYSTEM DROP BACKEND \"{}:{}\"", host, heartbeat_port)
        };

        tracing::info!(
            "Dropping {} node {}:{} from cluster {} (mode: {})",
            if self.cluster.is_shared_data() { "compute" } else { "backend" },
            host,
            heartbeat_port,
            self.cluster.name,
            self.cluster.deployment_mode
        );
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
}
