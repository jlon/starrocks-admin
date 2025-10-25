use crate::models::{Cluster, ClusterHealth, CreateClusterRequest, HealthCheck, HealthStatus, UpdateClusterRequest};
use crate::services::StarRocksClient;
use crate::utils::{ApiError, ApiResult};
use chrono::Utc;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ClusterService {
    pool: SqlitePool,
}

impl ClusterService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // Create a new cluster
    pub async fn create_cluster(
        &self,
        mut req: CreateClusterRequest,
        user_id: i64,
    ) -> ApiResult<Cluster> {
        // Clean input data - trim whitespace
        req.name = req.name.trim().to_string();
        req.fe_host = req.fe_host.trim().to_string();
        req.username = req.username.trim().to_string();
        req.catalog = req.catalog.trim().to_string();
        if let Some(ref mut desc) = req.description {
            *desc = desc.trim().to_string();
        }
        
        // Validate cleaned data
        if req.name.is_empty() {
            return Err(ApiError::validation_error("Cluster name cannot be empty"));
        }
        if req.fe_host.is_empty() {
            return Err(ApiError::validation_error("FE host cannot be empty"));
        }
        if req.username.is_empty() {
            return Err(ApiError::validation_error("Username cannot be empty"));
        }

        // Check if cluster name already exists
        let existing: Option<Cluster> = sqlx::query_as("SELECT * FROM clusters WHERE name = ?")
            .bind(&req.name)
            .fetch_optional(&self.pool)
            .await?;

        if existing.is_some() {
            return Err(ApiError::validation_error("Cluster name already exists"));
        }

        // Convert tags to JSON string
        let tags_json = req.tags.map(|t| serde_json::to_string(&t).unwrap_or_default());

        // Insert cluster (password is stored as-is for now, should be encrypted in production)
        let result = sqlx::query(
            "INSERT INTO clusters (name, description, fe_host, fe_http_port, fe_query_port, 
             username, password_encrypted, enable_ssl, connection_timeout, tags, catalog, created_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.fe_host)
        .bind(req.fe_http_port)
        .bind(req.fe_query_port)
        .bind(&req.username)
        .bind(&req.password)  // TODO: Encrypt in production
        .bind(req.enable_ssl)
        .bind(req.connection_timeout)
        .bind(&tags_json)
        .bind(&req.catalog)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        let cluster_id = result.last_insert_rowid();

        // Fetch and return the created cluster
        let cluster: Cluster = sqlx::query_as("SELECT * FROM clusters WHERE id = ?")
            .bind(cluster_id)
            .fetch_one(&self.pool)
            .await?;

        tracing::info!("Cluster created successfully: {} (ID: {})", cluster.name, cluster.id);
        tracing::debug!("Cluster details: host={}, port={}, ssl={}, catalog={}", 
                       cluster.fe_host, cluster.fe_http_port, cluster.enable_ssl, cluster.catalog);

        Ok(cluster)
    }

    // Get all clusters
    pub async fn list_clusters(&self) -> ApiResult<Vec<Cluster>> {
        let clusters: Vec<Cluster> = sqlx::query_as("SELECT * FROM clusters ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;

        Ok(clusters)
    }

    // Get cluster by ID
    pub async fn get_cluster(&self, cluster_id: i64) -> ApiResult<Cluster> {
        let cluster: Option<Cluster> = sqlx::query_as("SELECT * FROM clusters WHERE id = ?")
            .bind(cluster_id)
            .fetch_optional(&self.pool)
            .await?;

        cluster.ok_or_else(|| ApiError::cluster_not_found(cluster_id))
    }

    // Update cluster
    pub async fn update_cluster(
        &self,
        cluster_id: i64,
        req: UpdateClusterRequest,
    ) -> ApiResult<Cluster> {
        // Check if cluster exists
        let _cluster = self.get_cluster(cluster_id).await?;

        // Build dynamic SQL update query
        let mut updates = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(name) = &req.name {
            updates.push("name = ?");
            params.push(name.clone());
        }
        if let Some(desc) = &req.description {
            updates.push("description = ?");
            params.push(desc.clone());
        }
        if let Some(host) = &req.fe_host {
            updates.push("fe_host = ?");
            params.push(host.clone());
        }
        if let Some(http_port) = req.fe_http_port {
            updates.push("fe_http_port = ?");
            params.push(http_port.to_string());
        }
        if let Some(query_port) = req.fe_query_port {
            updates.push("fe_query_port = ?");
            params.push(query_port.to_string());
        }
        if let Some(username) = &req.username {
            updates.push("username = ?");
            params.push(username.clone());
        }
        if let Some(password) = &req.password {
            updates.push("password_encrypted = ?");
            params.push(password.clone());
        }
        if let Some(ssl) = req.enable_ssl {
            updates.push("enable_ssl = ?");
            params.push((ssl as i32).to_string());
        }
        if let Some(timeout) = req.connection_timeout {
            updates.push("connection_timeout = ?");
            params.push(timeout.to_string());
        }
        if let Some(tags) = &req.tags {
            updates.push("tags = ?");
            params.push(serde_json::to_string(tags).unwrap_or_default());
        }
        if let Some(catalog) = &req.catalog {
            updates.push("catalog = ?");
            params.push(catalog.clone());
        }

        if updates.is_empty() {
            return self.get_cluster(cluster_id).await;
        }

        updates.push("updated_at = CURRENT_TIMESTAMP");

        let sql = format!(
            "UPDATE clusters SET {} WHERE id = ?",
            updates.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for param in params {
            query = query.bind(param);
        }
        query = query.bind(cluster_id);

        query.execute(&self.pool).await?;

        tracing::info!("Cluster updated: ID {}", cluster_id);

        self.get_cluster(cluster_id).await
    }

    // Delete cluster
    pub async fn delete_cluster(&self, cluster_id: i64) -> ApiResult<()> {
        let result = sqlx::query("DELETE FROM clusters WHERE id = ?")
            .bind(cluster_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(ApiError::cluster_not_found(cluster_id));
        }

        tracing::info!("Cluster deleted: ID {}", cluster_id);

        Ok(())
    }

    // Get cluster health
    pub async fn get_cluster_health(&self, cluster_id: i64) -> ApiResult<ClusterHealth> {
        let cluster = self.get_cluster(cluster_id).await?;
        let client = StarRocksClient::new(cluster);

        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        // Check FE availability
        match client.get_runtime_info().await {
            Ok(_) => {
                checks.push(HealthCheck {
                    name: "FE Availability".to_string(),
                    status: "ok".to_string(),
                    message: "FE is reachable and responding".to_string(),
                });
            }
            Err(e) => {
                checks.push(HealthCheck {
                    name: "FE Availability".to_string(),
                    status: "critical".to_string(),
                    message: format!("FE is not reachable: {}", e),
                });
                overall_status = HealthStatus::Critical;
            }
        }

        // Check Backend nodes
        match client.get_backends().await {
            Ok(backends) => {
                let alive_count = backends.iter().filter(|b| b.alive == "true").count();
                let total_count = backends.len();

                if alive_count == total_count {
                    checks.push(HealthCheck {
                        name: "Backend Nodes".to_string(),
                        status: "ok".to_string(),
                        message: format!("All {} BE nodes are online", total_count),
                    });
                } else if alive_count > 0 {
                    checks.push(HealthCheck {
                        name: "Backend Nodes".to_string(),
                        status: "warning".to_string(),
                        message: format!(
                            "{}/{} BE nodes are online",
                            alive_count, total_count
                        ),
                    });
                    if overall_status == HealthStatus::Healthy {
                        overall_status = HealthStatus::Warning;
                    }
                } else {
                    checks.push(HealthCheck {
                        name: "Backend Nodes".to_string(),
                        status: "critical".to_string(),
                        message: "No BE nodes are online".to_string(),
                    });
                    overall_status = HealthStatus::Critical;
                }
            }
            Err(e) => {
                checks.push(HealthCheck {
                    name: "Backend Nodes".to_string(),
                    status: "warning".to_string(),
                    message: format!("Failed to check BE nodes: {}", e),
                });
                if overall_status == HealthStatus::Healthy {
                    overall_status = HealthStatus::Warning;
                }
            }
        }

        Ok(ClusterHealth {
            status: overall_status,
            checks,
            last_check_time: Utc::now(),
        })
    }

    // Get cluster health for a specific cluster object (used for testing new clusters)
    pub async fn get_cluster_health_for_cluster(
        &self,
        cluster: &Cluster,
        pool_manager: &crate::services::MySQLPoolManager,
    ) -> ApiResult<ClusterHealth> {
        use crate::services::MySQLClient;
        
        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        // Check connection by getting pool
        match pool_manager.get_pool(cluster).await {
            Ok(pool) => {
                let mysql_client = MySQLClient::from_pool(pool);
                
                // Test basic connection
                match mysql_client.query("SELECT 1").await {
                    Ok(_) => {
                        checks.push(HealthCheck {
                            name: "Database Connection".to_string(),
                            status: "ok".to_string(),
                            message: "Connection successful".to_string(),
                        });
                        
                        // Try to check FE availability via HTTP
                        let client = StarRocksClient::new(cluster.clone());
                        match client.get_runtime_info().await {
                            Ok(_) => {
                                checks.push(HealthCheck {
                                    name: "FE Availability".to_string(),
                                    status: "ok".to_string(),
                                    message: "FE is reachable and responding".to_string(),
                                });
                            }
                            Err(e) => {
                                checks.push(HealthCheck {
                                    name: "FE Availability".to_string(),
                                    status: "warning".to_string(),
                                    message: format!("FE HTTP check failed: {}", e),
                                });
                                if overall_status == HealthStatus::Healthy {
                                    overall_status = HealthStatus::Warning;
                                }
                            }
                        }
                        
                        // Try to check Backend nodes
                        match client.get_backends().await {
                            Ok(backends) => {
                                let alive_count = backends.iter().filter(|b| b.alive == "true").count();
                                let total_count = backends.len();

                                if total_count == 0 {
                                    checks.push(HealthCheck {
                                        name: "Backend Nodes".to_string(),
                                        status: "warning".to_string(),
                                        message: "No BE nodes found".to_string(),
                                    });
                                    if overall_status == HealthStatus::Healthy {
                                        overall_status = HealthStatus::Warning;
                                    }
                                } else if alive_count == total_count {
                                    checks.push(HealthCheck {
                                        name: "Backend Nodes".to_string(),
                                        status: "ok".to_string(),
                                        message: format!("All {} BE nodes are online", total_count),
                                    });
                                } else if alive_count > 0 {
                                    checks.push(HealthCheck {
                                        name: "Backend Nodes".to_string(),
                                        status: "warning".to_string(),
                                        message: format!(
                                            "{}/{} BE nodes are online",
                                            alive_count, total_count
                                        ),
                                    });
                                    if overall_status == HealthStatus::Healthy {
                                        overall_status = HealthStatus::Warning;
                                    }
                                } else {
                                    checks.push(HealthCheck {
                                        name: "Backend Nodes".to_string(),
                                        status: "critical".to_string(),
                                        message: "No BE nodes are online".to_string(),
                                    });
                                    overall_status = HealthStatus::Critical;
                                }
                            }
                            Err(e) => {
                                checks.push(HealthCheck {
                                    name: "Backend Nodes".to_string(),
                                    status: "warning".to_string(),
                                    message: format!("Failed to check BE nodes: {}", e),
                                });
                                if overall_status == HealthStatus::Healthy {
                                    overall_status = HealthStatus::Warning;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        checks.push(HealthCheck {
                            name: "Database Connection".to_string(),
                            status: "critical".to_string(),
                            message: format!("Connection failed: {}", e),
                        });
                        overall_status = HealthStatus::Critical;
                    }
                }
            }
            Err(e) => {
                checks.push(HealthCheck {
                    name: "Connection Pool".to_string(),
                    status: "critical".to_string(),
                    message: format!("Failed to create connection pool: {}", e),
                });
                overall_status = HealthStatus::Critical;
            }
        }

        Ok(ClusterHealth {
            status: overall_status,
            checks,
            last_check_time: Utc::now(),
        })
    }
}

