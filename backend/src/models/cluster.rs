use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Deployment mode for StarRocks cluster
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, sqlx::Type, Default)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum DeploymentMode {
    /// Shared-nothing architecture: BE nodes handle both storage and compute
    #[default]
    SharedNothing,
    /// Shared-data architecture: CN nodes for compute, separate object storage (S3/HDFS)
    SharedData,
}

impl std::fmt::Display for DeploymentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentMode::SharedNothing => write!(f, "shared_nothing"),
            DeploymentMode::SharedData => write!(f, "shared_data"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Cluster {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub fe_host: String,
    pub fe_http_port: i32,
    pub fe_query_port: i32,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_encrypted: String,
    pub enable_ssl: bool,
    pub connection_timeout: i32,
    pub tags: Option<String>,
    pub catalog: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<i64>,
    pub organization_id: Option<i64>,
    #[serde(default)]
    pub deployment_mode: DeploymentMode,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateClusterRequest {
    pub name: String,
    pub description: Option<String>,
    pub fe_host: String,
    #[serde(default = "default_http_port")]
    pub fe_http_port: i32,
    #[serde(default = "default_query_port")]
    pub fe_query_port: i32,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub enable_ssl: bool,
    #[serde(default = "default_timeout")]
    pub connection_timeout: i32,
    pub tags: Option<Vec<String>>,
    #[serde(default = "default_catalog")]
    pub catalog: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<i64>,
    #[serde(default)]
    pub deployment_mode: DeploymentMode,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateClusterRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fe_host: Option<String>,
    pub fe_http_port: Option<i32>,
    pub fe_query_port: Option<i32>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub enable_ssl: Option<bool>,
    pub connection_timeout: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub catalog: Option<String>,
    pub organization_id: Option<i64>,
    pub deployment_mode: Option<DeploymentMode>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClusterResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub fe_host: String,
    pub fe_http_port: i32,
    pub fe_query_port: i32,
    pub username: String,
    pub enable_ssl: bool,
    pub connection_timeout: i32,
    pub tags: Vec<String>,
    pub catalog: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<i64>,
    pub deployment_mode: DeploymentMode,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClusterHealth {
    pub status: HealthStatus,
    pub checks: Vec<HealthCheck>,
    pub last_check_time: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub message: String,
}

fn default_http_port() -> i32 {
    8030
}

fn default_query_port() -> i32 {
    9030
}

fn default_timeout() -> i32 {
    10
}

fn default_catalog() -> String {
    "default_catalog".to_string()
}

impl From<Cluster> for ClusterResponse {
    fn from(cluster: Cluster) -> Self {
        let tags = cluster
            .tags
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();

        Self {
            id: cluster.id,
            name: cluster.name,
            description: cluster.description,
            fe_host: cluster.fe_host,
            fe_http_port: cluster.fe_http_port,
            fe_query_port: cluster.fe_query_port,
            username: cluster.username,
            enable_ssl: cluster.enable_ssl,
            connection_timeout: cluster.connection_timeout,
            tags,
            catalog: cluster.catalog,
            is_active: cluster.is_active,
            created_at: cluster.created_at,
            updated_at: cluster.updated_at,
            organization_id: cluster.organization_id,
            deployment_mode: cluster.deployment_mode,
        }
    }
}

impl Cluster {
    /// Check if cluster is using shared-data (storage-compute separated) architecture
    pub fn is_shared_data(&self) -> bool {
        self.deployment_mode == DeploymentMode::SharedData
    }

    /// Check if cluster is using shared-nothing (storage-compute integrated) architecture
    pub fn is_shared_nothing(&self) -> bool {
        self.deployment_mode == DeploymentMode::SharedNothing
    }
}
