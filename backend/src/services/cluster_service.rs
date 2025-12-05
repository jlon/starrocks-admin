use crate::models::{
    Cluster, ClusterHealth, CreateClusterRequest, HealthCheck, HealthStatus, UpdateClusterRequest,
};
use crate::services::{MySQLPoolManager, StarRocksClient};
use crate::utils::{ApiError, ApiResult};
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct ClusterService {
    pool: SqlitePool,
    mysql_pool_manager: Arc<MySQLPoolManager>,
}

impl ClusterService {
    pub fn new(pool: SqlitePool, mysql_pool_manager: Arc<MySQLPoolManager>) -> Self {
        Self { pool, mysql_pool_manager }
    }

    // Create a new cluster
    pub async fn create_cluster(
        &self,
        mut req: CreateClusterRequest,
        user_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
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

        let target_org_id = self
            .resolve_target_org(req.organization_id, requestor_org, is_super_admin)
            .await?;

        // Convert tags to JSON string
        let tags_json = req
            .tags
            .map(|t| serde_json::to_string(&t).unwrap_or_default());

        // Check if this will be the first cluster within the organization
        let existing_cluster_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM clusters WHERE organization_id = ?")
                .bind(target_org_id)
                .fetch_one(&self.pool)
                .await?;

        let is_first_cluster = existing_cluster_count.0 == 0;

        // Insert cluster (password is stored as-is for now, should be encrypted in production)
        let result = sqlx::query(
            "INSERT INTO clusters (name, description, fe_host, fe_http_port, fe_query_port, 
             username, password_encrypted, enable_ssl, connection_timeout, tags, catalog, 
             is_active, created_by, organization_id, deployment_mode)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.fe_host)
        .bind(req.fe_http_port)
        .bind(req.fe_query_port)
        .bind(&req.username)
        .bind(&req.password) // TODO: Encrypt in production
        .bind(req.enable_ssl)
        .bind(req.connection_timeout)
        .bind(&tags_json)
        .bind(&req.catalog)
        .bind(if is_first_cluster { 1 } else { 0 }) // Set as active if first cluster in org
        .bind(user_id)
        .bind(target_org_id)
        .bind(req.deployment_mode.to_string())
        .execute(&self.pool)
        .await?;

        let cluster_id = result.last_insert_rowid();

        // Ensure each organization always has one active cluster if none exists
        if !is_first_cluster {
            let active_count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM clusters WHERE is_active = 1 AND organization_id = ?",
            )
            .bind(target_org_id)
            .fetch_one(&self.pool)
            .await?;

            if active_count.0 == 0 {
                sqlx::query("UPDATE clusters SET is_active = 1 WHERE id = ?")
                    .bind(cluster_id)
                    .execute(&self.pool)
                    .await?;
                tracing::info!(
                    "Automatically activated newly created cluster for organization {} (no active cluster existed)",
                    target_org_id
                );
            }
        }

        // Fetch and return the created cluster
        let cluster: Cluster = sqlx::query_as("SELECT * FROM clusters WHERE id = ?")
            .bind(cluster_id)
            .fetch_one(&self.pool)
            .await?;

        tracing::info!("Cluster created successfully: {} (ID: {})", cluster.name, cluster.id);
        tracing::debug!(
            "Cluster details: host={}, port={}, ssl={}, catalog={}, active={}",
            cluster.fe_host,
            cluster.fe_http_port,
            cluster.enable_ssl,
            cluster.catalog,
            cluster.is_active
        );

        Ok(cluster)
    }

    // Get all clusters (unfiltered, handlers apply org filter)
    pub async fn list_clusters(&self) -> ApiResult<Vec<Cluster>> {
        let clusters: Vec<Cluster> =
            sqlx::query_as("SELECT * FROM clusters ORDER BY created_at DESC")
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

    // Get the currently active cluster (global)
    pub async fn get_active_cluster(&self) -> ApiResult<Cluster> {
        let cluster: Option<Cluster> =
            sqlx::query_as("SELECT * FROM clusters WHERE is_active = 1 LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;

        cluster.ok_or_else(|| {
            ApiError::not_found("No active cluster found. Please activate a cluster first.")
        })
    }

    // Get active cluster scoped by organization
    pub async fn get_active_cluster_by_org(&self, org_id: Option<i64>) -> ApiResult<Cluster> {
        let cluster: Option<Cluster> = if let Some(org) = org_id {
            sqlx::query_as(
                "SELECT * FROM clusters WHERE is_active = 1 AND organization_id = ? LIMIT 1",
            )
            .bind(org)
            .fetch_optional(&self.pool)
            .await?
        } else {
            None
        };

        cluster.ok_or_else(|| {
            ApiError::not_found(
                "No active cluster found for your organization. Please activate a cluster first.",
            )
        })
    }

    // Set a cluster as active (deactivating all others in the same organization)
    pub async fn set_active_cluster(&self, cluster_id: i64) -> ApiResult<Cluster> {
        // Check if cluster exists and fetch its org
        let cluster = self.get_cluster(cluster_id).await?;
        let org_id = cluster.organization_id;

        // Start transaction to ensure atomicity
        let mut tx = self.pool.begin().await?;

        // First, deactivate all clusters in the same organization
        if let Some(org) = org_id {
            sqlx::query("UPDATE clusters SET is_active = 0 WHERE organization_id = ?")
                .bind(org)
                .execute(&mut *tx)
                .await?;
        } else {
            // If cluster has no org, deactivate all clusters with NULL org
            sqlx::query("UPDATE clusters SET is_active = 0 WHERE organization_id IS NULL")
                .execute(&mut *tx)
                .await?;
        }

        // Then, activate the target cluster
        sqlx::query(
            "UPDATE clusters SET is_active = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(cluster_id)
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        tracing::info!("Cluster activated: ID {} (org: {:?})", cluster_id, org_id);

        // Fetch and return the updated cluster
        self.get_cluster(cluster_id).await
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
        if let Some(org_id) = req.organization_id {
            updates.push("organization_id = ?");
            params.push(org_id.to_string());
        }
        if let Some(mode) = &req.deployment_mode {
            updates.push("deployment_mode = ?");
            params.push(mode.to_string());
        }

        if updates.is_empty() {
            return self.get_cluster(cluster_id).await;
        }

        updates.push("updated_at = CURRENT_TIMESTAMP");

        let sql = format!("UPDATE clusters SET {} WHERE id = ?", updates.join(", "));

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
        // Check if this is the active cluster and capture organization
        let cluster_record: Option<(bool, Option<i64>)> =
            sqlx::query_as("SELECT is_active, organization_id FROM clusters WHERE id = ?")
                .bind(cluster_id)
                .fetch_optional(&self.pool)
                .await?;

        let is_active = cluster_record.map(|r| r.0).unwrap_or(false);
        let cluster_org_id = cluster_record.and_then(|r| r.1);

        // Delete the cluster
        let result = sqlx::query("DELETE FROM clusters WHERE id = ?")
            .bind(cluster_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(ApiError::cluster_not_found(cluster_id));
        }

        tracing::info!("Cluster deleted: ID {}", cluster_id);

        // If we deleted the active cluster, activate another one (first by creation time)
        if is_active {
            let next_cluster: Option<(i64,)> = if let Some(org_id) = cluster_org_id {
                sqlx::query_as(
                    "SELECT id FROM clusters WHERE organization_id = ? ORDER BY created_at DESC LIMIT 1",
                )
                .bind(org_id)
                .fetch_optional(&self.pool)
                .await?
            } else {
                sqlx::query_as(
                    "SELECT id FROM clusters WHERE organization_id IS NULL ORDER BY created_at DESC LIMIT 1",
                )
                .fetch_optional(&self.pool)
                .await?
            };

            if let Some((next_id,)) = next_cluster {
                sqlx::query("UPDATE clusters SET is_active = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(next_id)
                    .execute(&self.pool)
                    .await?;
                tracing::info!("Automatically activated cluster ID {} after deletion", next_id);
            }
        }

        Ok(())
    }

    async fn resolve_target_org(
        &self,
        requested_org: Option<i64>,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<i64> {
        if is_super_admin {
            if let Some(id) = requested_org.or(requestor_org) {
                return Ok(id);
            }
            return self.fetch_default_org_id().await;
        }

        requestor_org.ok_or_else(|| {
            ApiError::forbidden("Organization context required for cluster operations")
        })
    }

    async fn fetch_default_org_id(&self) -> ApiResult<i64> {
        if let Some(id) =
            sqlx::query_scalar("SELECT id FROM organizations WHERE code = 'default_org'")
                .fetch_optional(&self.pool)
                .await?
        {
            return Ok(id);
        }

        sqlx::query("INSERT INTO organizations (code, name, description, is_system) VALUES ('default_org', 'Default Organization', 'Auto-created default organization', 1)")
            .execute(&self.pool)
            .await?;

        sqlx::query_scalar("SELECT id FROM organizations WHERE code = 'default_org'")
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Default organization not found"))
    }

    // Get cluster health
    pub async fn get_cluster_health(&self, cluster_id: i64) -> ApiResult<ClusterHealth> {
        let cluster = self.get_cluster(cluster_id).await?;
        let is_shared_data = cluster.is_shared_data();
        let client = StarRocksClient::new(cluster, self.mysql_pool_manager.clone());

        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        match client.get_frontends().await {
            Ok(frontends) => {
                let alive_count = frontends.iter().filter(|f| f.alive == "true").count();
                let total_count = frontends.len();

                if total_count == 0 {
                    checks.push(HealthCheck {
                        name: "Frontend Nodes".to_string(),
                        status: "critical".to_string(),
                        message: "No FE nodes found".to_string(),
                    });
                    overall_status = HealthStatus::Critical;
                } else if alive_count == total_count {
                    checks.push(HealthCheck {
                        name: "Frontend Nodes".to_string(),
                        status: "ok".to_string(),
                        message: format!("All {} FE nodes are online", total_count),
                    });
                } else if alive_count > 0 {
                    checks.push(HealthCheck {
                        name: "Frontend Nodes".to_string(),
                        status: "warning".to_string(),
                        message: format!("{}/{} FE nodes are online", alive_count, total_count),
                    });
                    if overall_status == HealthStatus::Healthy {
                        overall_status = HealthStatus::Warning;
                    }
                } else {
                    checks.push(HealthCheck {
                        name: "Frontend Nodes".to_string(),
                        status: "critical".to_string(),
                        message: "No FE nodes are online".to_string(),
                    });
                    overall_status = HealthStatus::Critical;
                }
            },
            Err(e) => {
                checks.push(HealthCheck {
                    name: "Frontend Nodes".to_string(),
                    status: "critical".to_string(),
                    message: format!("Failed to check FE nodes: {}", e),
                });
                overall_status = HealthStatus::Critical;
            },
        }

        // Check compute nodes (BE in shared-nothing, CN in shared-data)
        let node_type = if is_shared_data { "CN" } else { "BE" };
        match client.get_backends().await {
            Ok(backends) => {
                let alive_count = backends.iter().filter(|b| b.alive == "true").count();
                let total_count = backends.len();

                if alive_count == total_count {
                    checks.push(HealthCheck {
                        name: "Compute Nodes".to_string(),
                        status: "ok".to_string(),
                        message: format!("All {} {} nodes are online", total_count, node_type),
                    });
                } else if alive_count > 0 {
                    checks.push(HealthCheck {
                        name: "Compute Nodes".to_string(),
                        status: "warning".to_string(),
                        message: format!(
                            "{}/{} {} nodes are online",
                            alive_count, total_count, node_type
                        ),
                    });
                    if overall_status == HealthStatus::Healthy {
                        overall_status = HealthStatus::Warning;
                    }
                } else {
                    checks.push(HealthCheck {
                        name: "Compute Nodes".to_string(),
                        status: "critical".to_string(),
                        message: format!("No {} nodes are online", node_type),
                    });
                    overall_status = HealthStatus::Critical;
                }
            },
            Err(e) => {
                checks.push(HealthCheck {
                    name: "Compute Nodes".to_string(),
                    status: "warning".to_string(),
                    message: format!("Failed to check {} nodes: {}", node_type, e),
                });
                if overall_status == HealthStatus::Healthy {
                    overall_status = HealthStatus::Warning;
                }
            },
        }

        Ok(ClusterHealth { status: overall_status, checks, last_check_time: Utc::now() })
    }

    // Get cluster health for a specific cluster object (used for testing new clusters)
    pub async fn get_cluster_health_for_cluster(
        &self,
        cluster: &Cluster,
    ) -> ApiResult<ClusterHealth> {
        use crate::services::MySQLClient;

        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        // Check connection by getting pool
        match self.mysql_pool_manager.get_pool(cluster).await {
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
                        let client =
                            StarRocksClient::new(cluster.clone(), self.mysql_pool_manager.clone());
                        match client.get_runtime_info().await {
                            Ok(_) => {
                                checks.push(HealthCheck {
                                    name: "FE Availability".to_string(),
                                    status: "ok".to_string(),
                                    message: "FE is reachable and responding".to_string(),
                                });
                            },
                            Err(e) => {
                                checks.push(HealthCheck {
                                    name: "FE Availability".to_string(),
                                    status: "warning".to_string(),
                                    message: format!("FE HTTP check failed: {}", e),
                                });
                                if overall_status == HealthStatus::Healthy {
                                    overall_status = HealthStatus::Warning;
                                }
                            },
                        }

                        // Try to check compute nodes (BE in shared-nothing, CN in shared-data)
                        let node_type = if cluster.is_shared_data() { "CN" } else { "BE" };
                        match client.get_backends().await {
                            Ok(backends) => {
                                let alive_count =
                                    backends.iter().filter(|b| b.alive == "true").count();
                                let total_count = backends.len();

                                if total_count == 0 {
                                    checks.push(HealthCheck {
                                        name: "Compute Nodes".to_string(),
                                        status: "warning".to_string(),
                                        message: format!("No {} nodes found", node_type),
                                    });
                                    if overall_status == HealthStatus::Healthy {
                                        overall_status = HealthStatus::Warning;
                                    }
                                } else if alive_count == total_count {
                                    checks.push(HealthCheck {
                                        name: "Compute Nodes".to_string(),
                                        status: "ok".to_string(),
                                        message: format!(
                                            "All {} {} nodes are online",
                                            total_count, node_type
                                        ),
                                    });
                                } else if alive_count > 0 {
                                    checks.push(HealthCheck {
                                        name: "Compute Nodes".to_string(),
                                        status: "warning".to_string(),
                                        message: format!(
                                            "{}/{} {} nodes are online",
                                            alive_count, total_count, node_type
                                        ),
                                    });
                                    if overall_status == HealthStatus::Healthy {
                                        overall_status = HealthStatus::Warning;
                                    }
                                } else {
                                    checks.push(HealthCheck {
                                        name: "Compute Nodes".to_string(),
                                        status: "critical".to_string(),
                                        message: format!("No {} nodes are online", node_type),
                                    });
                                    overall_status = HealthStatus::Critical;
                                }
                            },
                            Err(e) => {
                                checks.push(HealthCheck {
                                    name: "Compute Nodes".to_string(),
                                    status: "warning".to_string(),
                                    message: format!("Failed to check {} nodes: {}", node_type, e),
                                });
                                if overall_status == HealthStatus::Healthy {
                                    overall_status = HealthStatus::Warning;
                                }
                            },
                        }
                    },
                    Err(e) => {
                        checks.push(HealthCheck {
                            name: "Database Connection".to_string(),
                            status: "critical".to_string(),
                            message: format!("Connection failed: {}", e),
                        });
                        overall_status = HealthStatus::Critical;
                    },
                }
            },
            Err(e) => {
                checks.push(HealthCheck {
                    name: "Connection Pool".to_string(),
                    status: "critical".to_string(),
                    message: format!("Failed to create connection pool: {}", e),
                });
                overall_status = HealthStatus::Critical;
            },
        }

        Ok(ClusterHealth { status: overall_status, checks, last_check_time: Utc::now() })
    }
}
