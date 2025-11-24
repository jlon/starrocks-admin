-- ========================================
-- StarRocks Admin - Unified Database Schema
-- ========================================
-- Created: 2025-01-25
-- Updated: 2025-01-28
-- Purpose: Complete database schema including all tables, cluster overview features, and global active cluster management
-- This migration consolidates all previous migrations into a single file

-- ========================================
-- SECTION 1: CORE TABLES
-- ========================================

-- ==============================================
-- 1.1 Users Table
-- ==============================================
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    email VARCHAR(100),
    avatar VARCHAR(255),    
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create index on username
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);

-- ==============================================
-- 1.2 Clusters Table (with is_active for global cluster activation)
-- ==============================================
CREATE TABLE IF NOT EXISTS clusters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    fe_host VARCHAR(255) NOT NULL,
    fe_http_port INTEGER NOT NULL DEFAULT 8030,
    fe_query_port INTEGER NOT NULL DEFAULT 9030,
    username VARCHAR(100) NOT NULL,
    password_encrypted VARCHAR(255) NOT NULL,
    enable_ssl BOOLEAN DEFAULT 0,
    connection_timeout INTEGER DEFAULT 10,
    tags TEXT,
    catalog VARCHAR(100) DEFAULT 'default_catalog',
    deployment_mode VARCHAR(20) DEFAULT 'shared_nothing' NOT NULL,
    is_active BOOLEAN DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by INTEGER
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_clusters_name ON clusters(name);
CREATE INDEX IF NOT EXISTS idx_clusters_is_active ON clusters(is_active);
CREATE INDEX IF NOT EXISTS idx_clusters_deployment_mode ON clusters(deployment_mode);

-- ==============================================
-- 1.3 Monitor History Table
-- ==============================================
CREATE TABLE IF NOT EXISTS monitor_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id INTEGER NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value TEXT NOT NULL,
    collected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE
);

-- Create indexes for faster queries
CREATE INDEX IF NOT EXISTS idx_monitor_history_cluster_metric ON monitor_history(cluster_id, metric_name);
CREATE INDEX IF NOT EXISTS idx_monitor_history_collected_at ON monitor_history(collected_at);

-- ==============================================
-- 1.4 System Functions Table
-- ==============================================
CREATE TABLE IF NOT EXISTS system_functions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id INTEGER NULL,                    -- NULL = system default (all clusters)
    category_name TEXT NOT NULL,
    function_name TEXT NOT NULL,
    description TEXT NOT NULL,
    sql_query TEXT NOT NULL,
    display_order INTEGER NOT NULL DEFAULT 0,
    category_order INTEGER NOT NULL DEFAULT 0,
    is_favorited BOOLEAN NOT NULL DEFAULT FALSE,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_by INTEGER NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_system_functions_cluster_id ON system_functions (cluster_id);
CREATE INDEX IF NOT EXISTS idx_system_functions_category_order ON system_functions (category_order);
CREATE INDEX IF NOT EXISTS idx_system_functions_display_order ON system_functions (display_order);
CREATE INDEX IF NOT EXISTS idx_system_functions_is_system ON system_functions (is_system);

-- ==============================================
-- 1.5 System Function Preferences Table
-- ==============================================
CREATE TABLE IF NOT EXISTS system_function_preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id INTEGER NOT NULL,
    function_id INTEGER NOT NULL,
    category_order INTEGER NOT NULL DEFAULT 0,
    display_order INTEGER NOT NULL DEFAULT 0,
    is_favorited BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(cluster_id, function_id),
    FOREIGN KEY (cluster_id) REFERENCES clusters (id) ON DELETE CASCADE,
    FOREIGN KEY (function_id) REFERENCES system_functions (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_preferences_cluster_id ON system_function_preferences (cluster_id);
CREATE INDEX IF NOT EXISTS idx_preferences_function_id ON system_function_preferences (function_id);
CREATE INDEX IF NOT EXISTS idx_preferences_cluster_function ON system_function_preferences (cluster_id, function_id);

-- ========================================
-- SECTION 2: OVERVIEW AND MONITORING TABLES
-- ========================================
-- Purpose: Support cluster overview dashboard with real-time and historical metrics
-- Reference: docs/ARCHITECTURE_ANALYSIS_AND_INTEGRATION.md

-- ==============================================
-- 2.1 Metrics Snapshots Table (High-frequency: 30s interval)
-- ==============================================
-- Purpose: Store real-time metrics snapshots every 30 seconds
-- Retention: 7 days (~20,160 records per cluster)
-- Data Source: StarRocks Prometheus metrics + Backend/Frontend info

CREATE TABLE IF NOT EXISTS metrics_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id INTEGER NOT NULL,
    collected_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- Query Performance Metrics
    qps REAL NOT NULL DEFAULT 0.0,                      -- Queries per second
    rps REAL NOT NULL DEFAULT 0.0,                      -- Requests per second
    query_latency_p50 REAL NOT NULL DEFAULT 0.0,        -- P50 latency (ms)
    query_latency_p95 REAL NOT NULL DEFAULT 0.0,        -- P95 latency (ms)
    query_latency_p99 REAL NOT NULL DEFAULT 0.0,        -- P99 latency (ms)
    query_total BIGINT NOT NULL DEFAULT 0,              -- Total queries count
    query_success BIGINT NOT NULL DEFAULT 0,            -- Successful queries
    query_error BIGINT NOT NULL DEFAULT 0,              -- Error queries
    query_timeout BIGINT NOT NULL DEFAULT 0,            -- Timeout queries
    
    -- Cluster Health Metrics
    backend_total INTEGER NOT NULL DEFAULT 0,           -- Total BE nodes
    backend_alive INTEGER NOT NULL DEFAULT 0,           -- Alive BE nodes
    frontend_total INTEGER NOT NULL DEFAULT 0,          -- Total FE nodes
    frontend_alive INTEGER NOT NULL DEFAULT 0,          -- Alive FE nodes
    
    -- Resource Usage Metrics
    total_cpu_usage REAL NOT NULL DEFAULT 0.0,          -- Total CPU usage (%)
    avg_cpu_usage REAL NOT NULL DEFAULT 0.0,            -- Average CPU usage (%)
    total_memory_usage REAL NOT NULL DEFAULT 0.0,       -- Total memory usage (%)
    avg_memory_usage REAL NOT NULL DEFAULT 0.0,         -- Average memory usage (%)
    disk_total_bytes BIGINT NOT NULL DEFAULT 0,         -- Total disk capacity (bytes)
    disk_used_bytes BIGINT NOT NULL DEFAULT 0,          -- Used disk space (bytes)
    disk_usage_pct REAL NOT NULL DEFAULT 0.0,           -- Disk usage percentage
    
    -- Storage Metrics
    tablet_count BIGINT NOT NULL DEFAULT 0,             -- Total tablet count
    max_compaction_score REAL NOT NULL DEFAULT 0.0,     -- Max compaction score
    
    -- Transaction Metrics
    txn_running INTEGER NOT NULL DEFAULT 0,             -- Running transactions
    txn_success_total BIGINT NOT NULL DEFAULT 0,        -- Total successful txns
    txn_failed_total BIGINT NOT NULL DEFAULT 0,         -- Total failed txns
    
    -- Load Metrics
    load_running INTEGER NOT NULL DEFAULT 0,            -- Running load jobs
    load_finished_total BIGINT NOT NULL DEFAULT 0,      -- Total finished loads
    
    -- JVM Metrics (FE)
    jvm_heap_total BIGINT NOT NULL DEFAULT 0,           -- JVM heap total (bytes)
    jvm_heap_used BIGINT NOT NULL DEFAULT 0,            -- JVM heap used (bytes)
    jvm_heap_usage_pct REAL NOT NULL DEFAULT 0.0,       -- JVM heap usage (%)
    jvm_thread_count INTEGER NOT NULL DEFAULT 0,        -- JVM thread count
    
    -- Network Metrics (BE)
    network_bytes_sent_total BIGINT NOT NULL DEFAULT 0, -- Total bytes sent (cumulative)
    network_bytes_received_total BIGINT NOT NULL DEFAULT 0, -- Total bytes received (cumulative)
    network_send_rate REAL NOT NULL DEFAULT 0.0,        -- Network send rate (bytes/sec)
    network_receive_rate REAL NOT NULL DEFAULT 0.0,     -- Network receive rate (bytes/sec)
    
    -- IO Metrics (BE)
    io_read_bytes_total BIGINT NOT NULL DEFAULT 0,      -- Total bytes read (cumulative)
    io_write_bytes_total BIGINT NOT NULL DEFAULT 0,     -- Total bytes written (cumulative)
    io_read_rate REAL NOT NULL DEFAULT 0.0,             -- Disk read rate (bytes/sec)
    io_write_rate REAL NOT NULL DEFAULT 0.0,            -- Disk write rate (bytes/sec)
    
    -- Raw Metrics (JSON format for flexibility)
    raw_metrics TEXT,                                   -- JSON: additional metrics
    
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE
);

-- Indexes for efficient time-series queries
CREATE INDEX IF NOT EXISTS idx_metrics_snapshots_cluster_time 
ON metrics_snapshots(cluster_id, collected_at DESC);

CREATE INDEX IF NOT EXISTS idx_metrics_snapshots_time 
ON metrics_snapshots(collected_at DESC);

-- ==============================================
-- 2.2 Daily Snapshots Table (Low-frequency: daily)
-- ==============================================
-- Purpose: Store daily aggregated statistics
-- Retention: 90 days (~90 records per cluster)
-- Update Frequency: Daily at midnight

CREATE TABLE IF NOT EXISTS daily_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id INTEGER NOT NULL,
    snapshot_date DATE NOT NULL,
    
    -- Aggregated Query Statistics
    avg_qps REAL NOT NULL DEFAULT 0.0,                  -- Average QPS of the day
    max_qps REAL NOT NULL DEFAULT 0.0,                  -- Peak QPS of the day
    min_qps REAL NOT NULL DEFAULT 0.0,                  -- Minimum QPS of the day
    avg_latency_p99 REAL NOT NULL DEFAULT 0.0,          -- Average P99 latency
    max_latency_p99 REAL NOT NULL DEFAULT 0.0,          -- Maximum P99 latency
    total_queries BIGINT NOT NULL DEFAULT 0,            -- Total queries of the day
    total_errors BIGINT NOT NULL DEFAULT 0,             -- Total errors of the day
    error_rate REAL NOT NULL DEFAULT 0.0,               -- Error rate (%)
    
    -- Aggregated Resource Statistics
    avg_cpu_usage REAL NOT NULL DEFAULT 0.0,            -- Average CPU usage
    max_cpu_usage REAL NOT NULL DEFAULT 0.0,            -- Peak CPU usage
    avg_memory_usage REAL NOT NULL DEFAULT 0.0,         -- Average memory usage
    max_memory_usage REAL NOT NULL DEFAULT 0.0,         -- Peak memory usage
    avg_disk_usage_pct REAL NOT NULL DEFAULT 0.0,       -- Average disk usage (percentage)
    max_disk_usage_pct REAL NOT NULL DEFAULT 0.0,       -- Peak disk usage (percentage)
    
    -- Availability Statistics
    avg_backend_alive REAL NOT NULL DEFAULT 0.0,        -- Average alive BE nodes
    min_backend_alive INTEGER NOT NULL DEFAULT 0,       -- Minimum alive BE nodes
    total_downtime_seconds INTEGER NOT NULL DEFAULT 0,  -- Total downtime (seconds)
    availability_pct REAL NOT NULL DEFAULT 100.0,       -- Availability percentage
    
    -- Data Growth Statistics
    data_size_start BIGINT NOT NULL DEFAULT 0,          -- Data size at day start
    data_size_end BIGINT NOT NULL DEFAULT 0,            -- Data size at day end
    data_growth_bytes BIGINT NOT NULL DEFAULT 0,        -- Data growth (bytes)
    data_growth_rate REAL NOT NULL DEFAULT 0.0,         -- Growth rate (%)
    
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE,
    UNIQUE(cluster_id, snapshot_date)
);

-- Indexes for daily queries
CREATE INDEX IF NOT EXISTS idx_daily_snapshots_cluster_date 
ON daily_snapshots(cluster_id, snapshot_date DESC);

CREATE INDEX IF NOT EXISTS idx_daily_snapshots_date 
ON daily_snapshots(snapshot_date DESC);

-- ==============================================
-- 2.3 Data Statistics Cache Table (On-demand update)
-- ==============================================
-- Purpose: Cache expensive queries (database/table counts, top tables)
-- Update Frequency: Every 5-10 minutes or on-demand
-- This reduces query pressure on StarRocks

CREATE TABLE IF NOT EXISTS data_statistics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id INTEGER NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- Database/Table Statistics
    database_count INTEGER NOT NULL DEFAULT 0,          -- Total databases
    table_count INTEGER NOT NULL DEFAULT 0,             -- Total tables
    total_data_size BIGINT NOT NULL DEFAULT 0,          -- Total data size (bytes)
    total_index_size BIGINT NOT NULL DEFAULT 0,         -- Total index size (bytes)
    
    -- Top Tables by Size (JSON Array)
    -- Format: [{"database": "db1", "table": "t1", "size": 1024000, "rows": 1000}, ...]
    top_tables_by_size TEXT,
    
    -- Top Tables by Access Count (JSON Array)
    -- Format: [{"database": "db1", "table": "t1", "access_count": 500, "last_access": "2025-01-24T10:00:00Z"}, ...]
    top_tables_by_access TEXT,
    
    -- Materialized View Statistics
    mv_total INTEGER NOT NULL DEFAULT 0,                -- Total MVs
    mv_running INTEGER NOT NULL DEFAULT 0,              -- Running MVs
    mv_failed INTEGER NOT NULL DEFAULT 0,               -- Failed MVs
    mv_success INTEGER NOT NULL DEFAULT 0,              -- Successful MVs
    
    -- Schema Change Statistics
    schema_change_running INTEGER NOT NULL DEFAULT 0,   -- Running schema changes
    schema_change_pending INTEGER NOT NULL DEFAULT 0,   -- Pending schema changes
    schema_change_finished INTEGER NOT NULL DEFAULT 0,  -- Finished schema changes
    schema_change_failed INTEGER NOT NULL DEFAULT 0,    -- Failed schema changes
    
    -- Active Users Statistics
    active_users_1h INTEGER NOT NULL DEFAULT 0,         -- Active users in last 1 hour
    active_users_24h INTEGER NOT NULL DEFAULT 0,        -- Active users in last 24 hours
    unique_users TEXT,                                  -- JSON: list of unique users
    
    -- Query Statistics Cache
    slow_query_count_1h INTEGER NOT NULL DEFAULT 0,     -- Slow queries in last 1 hour
    slow_query_count_24h INTEGER NOT NULL DEFAULT 0,    -- Slow queries in last 24 hours
    
    FOREIGN KEY (cluster_id) REFERENCES clusters(id) ON DELETE CASCADE,
    UNIQUE(cluster_id)
);

-- Indexes for quick lookup
CREATE INDEX IF NOT EXISTS idx_data_statistics_cluster 
ON data_statistics(cluster_id);

CREATE INDEX IF NOT EXISTS idx_data_statistics_updated 
ON data_statistics(updated_at DESC);

-- ========================================
-- SECTION 3: DEFAULT DATA
-- ========================================

-- ==============================================
-- 3.1 Insert Default Admin User
-- ==============================================
-- Password: admin (bcrypt hash with DEFAULT_COST=12)
-- Hash generated using: bcrypt::hash("admin", DEFAULT_COST)
INSERT OR IGNORE INTO users (username, password_hash, email)
VALUES ('admin', '$2b$12$LFxvzXbmyBPO9Zp.1MFU4OX3fb8kID8AHYHklokkZvgyzmHuRTc56', 'admin@example.com');

-- ==============================================
-- 3.2 Insert Default System Functions
-- ==============================================
-- System default functions (cluster_id = NULL, applies to all clusters)
INSERT OR IGNORE INTO system_functions (cluster_id, category_name, function_name, description, sql_query, display_order, category_order, is_favorited, is_system, created_by, created_at, updated_at) VALUES
-- Database Management
(NULL, '数据库管理', 'tables', '表信息', 'HTTP_QUERY', 0, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '数据库管理', 'dbs', '数据库信息', 'HTTP_QUERY', 1, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '数据库管理', 'tablet_schema', 'Tablet Schema', 'HTTP_QUERY', 2, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '数据库管理', 'partitions', '分区信息', 'HTTP_QUERY', 3, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- Cluster Information
(NULL, '集群信息', 'backends', 'Backend节点信息', 'HTTP_QUERY', 0, 1, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '集群信息', 'frontends', 'Frontend节点信息', 'HTTP_QUERY', 1, 1, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '集群信息', 'brokers', 'Broker节点信息', 'HTTP_QUERY', 2, 1, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '集群信息', 'statistic', '统计信息', 'HTTP_QUERY', 3, 1, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- Transaction Management
(NULL, '事务管理', 'transactions', '事务信息', 'HTTP_QUERY', 0, 2, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- Task Management
(NULL, '任务管理', 'routine_loads', 'Routine Load任务', 'HTTP_QUERY', 0, 3, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '任务管理', 'stream_loads', 'Stream Load任务', 'HTTP_QUERY', 1, 3, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '任务管理', 'loads', 'Load任务', 'HTTP_QUERY', 2, 3, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '任务管理', 'load_error_hub', 'Load错误信息', 'HTTP_QUERY', 3, 3, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- Metadata Management
(NULL, '元数据管理', 'catalog', 'Catalog信息', 'HTTP_QUERY', 0, 4, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '元数据管理', 'resources', '资源信息', 'HTTP_QUERY', 1, 4, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '元数据管理', 'workgroups', '工作组信息', 'HTTP_QUERY', 2, 4, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '元数据管理', 'warehouses', '数据仓库信息', 'HTTP_QUERY', 3, 4, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- Storage Management
(NULL, '存储管理', 'tablets', 'Tablet信息', 'HTTP_QUERY', 0, 5, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '存储管理', 'colocate_group', 'Colocate Group信息', 'HTTP_QUERY', 1, 5, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '存储管理', 'compactions', '压缩任务信息', 'HTTP_QUERY', 2, 5, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '存储管理', 'routine_load_jobs', 'Routine Load作业信息', 'HTTP_QUERY', 3, 5, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),

-- Job Management
(NULL, '作业管理', 'jobs', '作业信息', 'HTTP_QUERY', 0, 6, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '作业管理', 'tasks', '任务信息', 'HTTP_QUERY', 1, 6, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '作业管理', 'routine_load_jobs', 'Routine Load作业', 'HTTP_QUERY', 2, 6, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '作业管理', 'stream_load_jobs', 'Stream Load作业', 'HTTP_QUERY', 3, 6, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- ========================================
-- MIGRATION COMPLETE
-- ========================================
-- Tables Summary:
-- 
-- Core Tables (5):
--   1. users                         - User authentication
--   2. clusters                      - Cluster configuration
--   3. monitor_history              - Legacy monitoring (kept for compatibility)
--   4. system_functions             - System function definitions
--   5. system_function_preferences  - User preferences for functions
--
-- Cluster Overview Tables (3):
--   6. metrics_snapshots            - Real-time metrics (30s, 7-day retention)
--   7. daily_snapshots              - Daily aggregations (90-day retention)
--   8. data_statistics              - Cached statistics (on-demand update)
--
-- Default Data:
--   - Admin user (username: admin, password: admin)
--   - 28 system functions across 6 categories
--
-- Next Steps:
--   1. Run this migration: cargo sqlx migrate run
--   2. Start backend service to activate MetricsCollectorService
--   3. Access Cluster Overview at /pages/starrocks/overview
--
