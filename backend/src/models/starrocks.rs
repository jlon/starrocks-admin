use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TableObjectType {
    Table,
    View,
    MaterializedView,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct TableMetadata {
    pub name: String,
    pub object_type: TableObjectType,
}

// Helper function to deserialize string to i64
fn deserialize_string_to_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

// Helper function to deserialize string to i32
fn deserialize_string_to_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

// Helper function for default empty string
fn default_empty_string() -> String {
    "0".to_string()
}

// Backend node information (also used for Compute Nodes in shared-data architecture)
//
// Field definitions from StarRocks source code:
//
// /backends (BackendsProcDir.java) TITLE_NAMES:
//   BackendId, IP, HeartbeatPort, BePort, HttpPort, BrpcPort, LastStartTime, LastHeartbeat,
//   Alive, SystemDecommissioned, ClusterDecommissioned, TabletNum,
//   DataUsedCapacity, AvailCapacity, TotalCapacity, UsedPct,
//   MaxDiskUsedPct, ErrMsg, Version, Status, DataTotalCapacity,
//   DataUsedPct, CpuCores, MemLimit, NumRunningQueries, MemUsedPct, CpuUsedPct,
//   DataCacheMetrics, Location, StatusCode
//   + Shared-Data: StarletPort, WorkerId, WarehouseName
//
// /compute_nodes (ComputeNodeProcDir.java) TITLE_NAMES:
//   ComputeNodeId, IP, HeartbeatPort, BePort, HttpPort, BrpcPort, LastStartTime, LastHeartbeat,
//   Alive, SystemDecommissioned, ClusterDecommissioned, ErrMsg, Version,
//   CpuCores, MemLimit, NumRunningQueries, MemUsedPct, CpuUsedPct,
//   DataCacheMetrics, HasStoragePath, StatusCode
//   + Shared-Data: StarletPort, WorkerId, WarehouseName, TabletNum
//
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Backend {
    // Node ID: BackendId for BE, ComputeNodeId for CN
    // Use rename for serialization (output as BackendId), alias for deserialization (accept ComputeNodeId)
    #[serde(rename = "BackendId", alias = "ComputeNodeId", default = "default_empty_string")]
    pub backend_id: String,

    // IP address
    #[serde(rename = "IP", default = "default_empty_string")]
    pub host: String,

    // Heartbeat port
    #[serde(rename = "HeartbeatPort", default = "default_empty_string")]
    pub heartbeat_port: String,

    // BE port
    #[serde(rename = "BePort", default = "default_empty_string")]
    pub be_port: String,

    // HTTP port
    #[serde(rename = "HttpPort", default = "default_empty_string")]
    pub http_port: String,

    // BRPC port
    #[serde(rename = "BrpcPort", default = "default_empty_string")]
    pub brpc_port: String,

    // Last start time
    #[serde(rename = "LastStartTime", default = "default_empty_string")]
    pub last_start_time: String,

    // Last heartbeat time
    #[serde(rename = "LastHeartbeat", default = "default_empty_string")]
    pub last_heartbeat: String,

    // Alive status
    #[serde(rename = "Alive", default = "default_empty_string")]
    pub alive: String,

    // System decommissioned status
    #[serde(rename = "SystemDecommissioned", default = "default_empty_string")]
    pub system_decommissioned: String,

    // Cluster decommissioned status
    #[serde(rename = "ClusterDecommissioned", default = "default_empty_string")]
    pub cluster_decommissioned: String,

    // Tablet count (BE: local tablets, CN in shared-data: remote tablets)
    #[serde(rename = "TabletNum", default = "default_empty_string")]
    pub tablet_num: String,

    // Data used capacity (BE only)
    #[serde(rename = "DataUsedCapacity", default = "default_empty_string")]
    pub data_used_capacity: String,

    // Available capacity (BE only)
    #[serde(rename = "AvailCapacity", default = "default_empty_string")]
    pub avail_capacity: String,

    // Total capacity (BE only)
    #[serde(rename = "TotalCapacity", default = "default_empty_string")]
    pub total_capacity: String,

    // Used percentage (BE only)
    #[serde(rename = "UsedPct", default = "default_empty_string")]
    pub used_pct: String,

    // Max disk used percentage (BE only)
    #[serde(rename = "MaxDiskUsedPct", default = "default_empty_string")]
    pub max_disk_used_pct: String,

    // Error message
    #[serde(rename = "ErrMsg", default = "default_empty_string")]
    pub err_msg: String,

    // Version
    #[serde(rename = "Version", default = "default_empty_string")]
    pub version: String,

    // Status (BE only, JSON format)
    #[serde(rename = "Status", default = "default_empty_string")]
    pub status: String,

    // Data total capacity (BE only)
    #[serde(rename = "DataTotalCapacity", default = "default_empty_string")]
    pub data_total_capacity: String,

    // Data used percentage (BE only)
    #[serde(rename = "DataUsedPct", default = "default_empty_string")]
    pub data_used_pct: String,

    // CPU cores
    #[serde(rename = "CpuCores", default = "default_empty_string")]
    pub cpu_cores: String,

    // Memory limit
    #[serde(rename = "MemLimit", default = "default_empty_string")]
    pub mem_limit: String,

    // Number of running queries
    #[serde(rename = "NumRunningQueries", default = "default_empty_string")]
    pub num_running_queries: String,

    // Memory used percentage
    #[serde(rename = "MemUsedPct", default = "default_empty_string")]
    pub mem_used_pct: String,

    // CPU used percentage
    #[serde(rename = "CpuUsedPct", default = "default_empty_string")]
    pub cpu_used_pct: String,

    // Data cache metrics
    #[serde(rename = "DataCacheMetrics", default = "default_empty_string")]
    pub data_cache_metrics: String,

    // Location (BE only)
    #[serde(rename = "Location", default = "default_empty_string")]
    pub location: String,

    // Status code
    #[serde(rename = "StatusCode", default = "default_empty_string")]
    pub status_code: String,

    // Has storage path (CN only)
    #[serde(rename = "HasStoragePath", default = "default_empty_string")]
    pub has_storage_path: String,

    // Starlet port (Shared-Data mode)
    #[serde(rename = "StarletPort", default = "default_empty_string")]
    pub starlet_port: String,

    // Worker ID (Shared-Data mode)
    #[serde(rename = "WorkerId", default = "default_empty_string")]
    pub worker_id: String,

    // Warehouse name (Shared-Data mode)
    #[serde(rename = "WarehouseName", default = "default_empty_string")]
    pub warehouse_name: String,
}

// Frontend node information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Frontend {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "IP", alias = "Host")] // Support both IP and Host
    pub host: String,
    #[serde(rename = "EditLogPort")]
    pub edit_log_port: String,
    #[serde(rename = "HttpPort")]
    pub http_port: String,
    #[serde(rename = "QueryPort")]
    pub query_port: String,
    #[serde(rename = "RpcPort")]
    pub rpc_port: String,
    #[serde(rename = "Role")]
    pub role: String,
    #[serde(rename = "IsMaster", default)] // IsMaster field, optional
    pub is_master: Option<String>,
    #[serde(rename = "ClusterId")]
    pub cluster_id: String,
    #[serde(rename = "Join")]
    pub join: String,
    #[serde(rename = "Alive")]
    pub alive: String,
    #[serde(rename = "ReplayedJournalId")]
    pub replayed_journal_id: String,
    #[serde(rename = "LastHeartbeat")]
    pub last_heartbeat: String,
    #[serde(rename = "ErrMsg")]
    pub err_msg: String,
    #[serde(rename = "Version")]
    pub version: String,
    // New fields in StarRocks 3.5.2
    #[serde(rename = "IsHelper", default)]
    pub is_helper: Option<String>,
    #[serde(rename = "StartTime", default)]
    pub start_time: Option<String>,
}

// Query information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Query {
    #[serde(rename = "QueryId")]
    pub query_id: String,
    #[serde(rename = "ConnectionId")]
    pub connection_id: String,
    #[serde(rename = "Database", default)]
    pub database: String,
    #[serde(rename = "User")]
    pub user: String,
    #[serde(rename = "ScanBytes", default)]
    pub scan_bytes: String,
    #[serde(rename = "ProcessRows", default)]
    #[serde(alias = "ScanRows")] // Support both ProcessRows and ScanRows
    pub process_rows: String,
    #[serde(rename = "CPUTime", default)]
    pub cpu_time: String,
    #[serde(rename = "ExecTime", default)]
    pub exec_time: String,
    #[serde(rename = "Sql", default)]
    pub sql: String,
    // Additional fields from SHOW PROC '/current_queries'
    #[serde(rename = "StartTime", default)]
    pub start_time: Option<String>,
    #[serde(rename = "feIp", default)]
    pub fe_ip: Option<String>,
    #[serde(rename = "MemoryUsage", default)]
    pub memory_usage: Option<String>,
    #[serde(rename = "DiskSpillSize", default)]
    pub disk_spill_size: Option<String>,
    #[serde(rename = "ExecProgress", default)]
    pub exec_progress: Option<String>,
    #[serde(rename = "Warehouse", default)]
    pub warehouse: Option<String>,
    #[serde(rename = "CustomQueryId", default)]
    pub custom_query_id: Option<String>,
    #[serde(rename = "ResourceGroup", default)]
    pub resource_group: Option<String>,
}

// Session/Process information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Session {
    pub id: String,
    pub user: String,
    pub host: String,
    pub db: Option<String>,
    pub command: String,
    pub time: String,
    pub state: String,
    pub info: Option<String>,
}

// Variable information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Variable {
    pub name: String,
    pub value: String,
}

// Variable update request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateVariableRequest {
    pub value: String,
    #[serde(default = "default_scope")]
    pub scope: String, // "GLOBAL" or "SESSION"
}

fn default_scope() -> String {
    "GLOBAL".to_string()
}

// Finished (historical) query item sourced from audit tables
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QueryHistoryItem {
    pub query_id: String,
    pub user: String,
    #[serde(default)]
    pub default_db: String,
    pub sql_statement: String,
    pub query_type: String,
    pub start_time: String,
    #[serde(default)]
    pub end_time: String,
    /// total time in milliseconds (raw), frontend may format
    pub total_ms: i64,
    pub query_state: String,
    #[serde(default)]
    pub warehouse: String,
}

// Paginated query history response
#[derive(Debug, Serialize, ToSchema)]
pub struct QueryHistoryResponse {
    pub data: Vec<QueryHistoryItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

// System runtime information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RuntimeInfo {
    #[serde(default)]
    pub fe_node: String,
    #[serde(deserialize_with = "deserialize_string_to_i64")]
    pub total_mem: i64,
    #[serde(deserialize_with = "deserialize_string_to_i64")]
    pub free_mem: i64,
    #[serde(deserialize_with = "deserialize_string_to_i32")]
    pub thread_cnt: i32,
}

// Metrics summary
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricsSummary {
    // Query metrics
    pub qps: f64,
    pub rps: f64,
    pub query_total: i64,
    pub query_success: i64,
    pub query_err: i64,
    pub query_timeout: i64,
    pub query_err_rate: f64,
    pub query_latency_p50: f64,
    pub query_latency_p95: f64,
    pub query_latency_p99: f64,

    // FE system metrics
    pub jvm_heap_total: i64,
    pub jvm_heap_used: i64,
    pub jvm_heap_usage_pct: f64,
    pub jvm_thread_count: i32,

    // Backend aggregate metrics
    pub backend_total: usize,
    pub backend_alive: usize,
    pub tablet_count: i64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_usage_pct: f64,
    pub avg_cpu_usage_pct: f64,
    pub avg_mem_usage_pct: f64,
    pub total_running_queries: i32,

    // Storage metrics
    pub max_compaction_score: f64,

    // Transaction metrics
    pub txn_begin: i64,
    pub txn_success: i64,
    pub txn_failed: i64,

    // Load metrics
    pub load_finished: i64,
    pub routine_load_rows: i64,
}

// Query execute request
#[derive(Debug, Deserialize, ToSchema)]
pub struct QueryExecuteRequest {
    pub sql: String,
    #[serde(default = "default_limit")]
    pub limit: Option<i32>, // Optional limit, default 1000
    #[serde(default)]
    pub catalog: Option<String>, // Optional catalog name
    #[serde(default)]
    pub database: Option<String>, // Optional database name, will execute USE database before SQL
}

fn default_limit() -> Option<i32> {
    Some(1000)
}

// Single query execution result
#[derive(Debug, Serialize, ToSchema)]
pub struct SingleQueryResult {
    pub sql: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub execution_time_ms: u128,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// Query execute response (unified structure for single or multiple SQL)
#[derive(Debug, Serialize, ToSchema)]
pub struct QueryExecuteResponse {
    pub results: Vec<SingleQueryResult>,
    pub total_execution_time_ms: u128,
}

// Profile list item from SHOW PROFILELIST
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProfileListItem {
    #[serde(rename = "QueryId")]
    pub query_id: String,
    #[serde(rename = "StartTime")]
    pub start_time: String,
    #[serde(rename = "Time")]
    pub time: String,
    #[serde(rename = "State")]
    pub state: String,
    #[serde(rename = "Statement")]
    pub statement: String,
}

// Profile detail from get_query_profile()
#[derive(Debug, Serialize, ToSchema)]
pub struct ProfileDetail {
    pub query_id: String,
    pub profile_content: String,
}

// Catalog with its databases
#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogWithDatabases {
    pub catalog: String,
    pub databases: Vec<String>,
}

// Response containing all catalogs with their databases
#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogsWithDatabasesResponse {
    pub catalogs: Vec<CatalogWithDatabases>,
}

// SQL Blacklist item (mapped from SHOW SQLBLACKLIST output)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SqlBlacklistItem {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Pattern")]
    pub pattern: String,
}

// Request to add SQL blacklist
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSqlBlacklistRequest {
    pub pattern: String,
}
