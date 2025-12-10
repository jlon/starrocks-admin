-- ========================================
-- StarRocks Admin - Fix System Functions PROC Paths
-- ========================================
-- Created: 2025-01-30
-- Purpose: Remove unsupported PROC paths and add missing supported ones
-- 
-- StarRocks Official PROC Paths (25):
--   brokers, frontends, routine_loads, catalog, colocation_group, cluster_balance,
--   load_error_hub, meta_recovery, global_current_queries, tasks, compute_nodes,
--   statistic, jobs, warehouses, resources, monitor, transactions, backends,
--   current_queries, stream_loads, replications, dbs, current_backend_instances,
--   historical_nodes, compactions
--
-- Removed (unsupported):
--   tables, tablet_schema, partitions, loads, workgroups, tablets,
--   colocate_group (typo - should be colocation_group), routine_load_jobs, stream_load_jobs

-- ==============================================
-- 1. Delete unsupported system functions
-- ==============================================
DELETE FROM system_functions WHERE function_name IN (
    'tables',
    'tablet_schema', 
    'partitions',
    'loads',
    'workgroups',
    'tablets',
    'colocate_group',      -- typo, should be colocation_group
    'routine_load_jobs',
    'stream_load_jobs'
) AND is_system = 1;

-- ==============================================
-- 2. Remove unsupported permissions from role_permissions
-- ==============================================
DELETE FROM role_permissions WHERE permission_id IN (
    SELECT id FROM permissions WHERE code IN (
        'api:clusters:system:tables',
        'api:clusters:system:tablet_schema',
        'api:clusters:system:partitions',
        'api:clusters:system:loads',
        'api:clusters:system:workgroups',
        'api:clusters:system:tablets',
        'api:clusters:system:colocate_group',
        'api:clusters:system:routine_load_jobs',
        'api:clusters:system:stream_load_jobs'
    )
);

-- ==============================================
-- 3. Delete unsupported permissions
-- ==============================================
DELETE FROM permissions WHERE code IN (
    'api:clusters:system:tables',
    'api:clusters:system:tablet_schema',
    'api:clusters:system:partitions',
    'api:clusters:system:loads',
    'api:clusters:system:workgroups',
    'api:clusters:system:tablets',
    'api:clusters:system:colocate_group',
    'api:clusters:system:routine_load_jobs',
    'api:clusters:system:stream_load_jobs'
);

-- ==============================================
-- 4. Add missing supported permissions
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('api:clusters:system:monitor', '查询监控信息', 'api', 'clusters', 'system:monitor', 'GET /api/clusters/system/monitor'),
('api:clusters:system:cluster_balance', '查询集群均衡', 'api', 'clusters', 'system:cluster_balance', 'GET /api/clusters/system/cluster_balance'),
('api:clusters:system:historical_nodes', '查询历史节点', 'api', 'clusters', 'system:historical_nodes', 'GET /api/clusters/system/historical_nodes'),
('api:clusters:system:current_queries', '查询当前查询', 'api', 'clusters', 'system:current_queries', 'GET /api/clusters/system/current_queries'),
('api:clusters:system:global_current_queries', '查询全局当前查询', 'api', 'clusters', 'system:global_current_queries', 'GET /api/clusters/system/global_current_queries'),
('api:clusters:system:current_backend_instances', '查询当前后端实例', 'api', 'clusters', 'system:current_backend_instances', 'GET /api/clusters/system/current_backend_instances'),
('api:clusters:system:replications', '查询复制任务', 'api', 'clusters', 'system:replications', 'GET /api/clusters/system/replications'),
('api:clusters:system:meta_recovery', '查询元数据恢复', 'api', 'clusters', 'system:meta_recovery', 'GET /api/clusters/system/meta_recovery'),
('api:clusters:system:colocation_group', '查询Colocation Group', 'api', 'clusters', 'system:colocation_group', 'GET /api/clusters/system/colocation_group');

-- ==============================================
-- 5. Set parent_id for new permissions (link to menu:system)
-- ==============================================
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system')
WHERE code IN (
    'api:clusters:system:monitor',
    'api:clusters:system:cluster_balance',
    'api:clusters:system:historical_nodes',
    'api:clusters:system:current_queries',
    'api:clusters:system:global_current_queries',
    'api:clusters:system:current_backend_instances',
    'api:clusters:system:replications',
    'api:clusters:system:meta_recovery',
    'api:clusters:system:colocation_group'
);

-- ==============================================
-- 6. Assign new permissions to admin role
-- ==============================================
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id 
FROM permissions 
WHERE code IN (
    'api:clusters:system:monitor',
    'api:clusters:system:cluster_balance',
    'api:clusters:system:historical_nodes',
    'api:clusters:system:current_queries',
    'api:clusters:system:global_current_queries',
    'api:clusters:system:current_backend_instances',
    'api:clusters:system:replications',
    'api:clusters:system:meta_recovery',
    'api:clusters:system:colocation_group'
);

-- ==============================================
-- 2. Insert missing supported system functions
-- ==============================================
-- Only insert if not exists (using INSERT OR IGNORE)

-- Cluster Information
INSERT OR IGNORE INTO system_functions (cluster_id, category_name, function_name, description, sql_query, display_order, category_order, is_favorited, is_system, created_by, created_at, updated_at) VALUES
(NULL, '集群信息', 'compute_nodes', '计算节点信息(CN)', 'HTTP_QUERY', 1, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '集群信息', 'monitor', '监控信息', 'HTTP_QUERY', 5, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '集群信息', 'cluster_balance', '集群均衡', 'HTTP_QUERY', 6, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '集群信息', 'historical_nodes', '历史节点', 'HTTP_QUERY', 7, 0, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- Task Management
INSERT OR IGNORE INTO system_functions (cluster_id, category_name, function_name, description, sql_query, display_order, category_order, is_favorited, is_system, created_by, created_at, updated_at) VALUES
(NULL, '任务管理', 'tasks', '任务列表', 'HTTP_QUERY', 3, 3, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '任务管理', 'replications', '复制任务', 'HTTP_QUERY', 4, 3, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- Query Management
INSERT OR IGNORE INTO system_functions (cluster_id, category_name, function_name, description, sql_query, display_order, category_order, is_favorited, is_system, created_by, created_at, updated_at) VALUES
(NULL, '查询管理', 'current_queries', '当前查询', 'HTTP_QUERY', 0, 4, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '查询管理', 'global_current_queries', '全局当前查询', 'HTTP_QUERY', 1, 4, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
(NULL, '查询管理', 'current_backend_instances', '当前后端实例', 'HTTP_QUERY', 2, 4, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- Storage Management - fix colocation_group typo
INSERT OR IGNORE INTO system_functions (cluster_id, category_name, function_name, description, sql_query, display_order, category_order, is_favorited, is_system, created_by, created_at, updated_at) VALUES
(NULL, '存储管理', 'colocation_group', 'Colocation Group', 'HTTP_QUERY', 1, 6, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- System Maintenance
INSERT OR IGNORE INTO system_functions (cluster_id, category_name, function_name, description, sql_query, display_order, category_order, is_favorited, is_system, created_by, created_at, updated_at) VALUES
(NULL, '系统维护', 'meta_recovery', '元数据恢复', 'HTTP_QUERY', 0, 8, false, true, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- ==============================================
-- 3. Update category names for consistency
-- ==============================================
-- Move catalog from 元数据管理 to 数据库管理
UPDATE system_functions SET category_name = '数据库管理', category_order = 1, display_order = 1 
WHERE function_name = 'catalog' AND is_system = 1;

-- Update resources to 资源管理
UPDATE system_functions SET category_name = '资源管理', category_order = 5, display_order = 0 
WHERE function_name = 'resources' AND is_system = 1;

-- Update warehouses to 资源管理
UPDATE system_functions SET category_name = '资源管理', category_order = 5, display_order = 1 
WHERE function_name = 'warehouses' AND is_system = 1;

-- Update compactions to 存储管理
UPDATE system_functions SET category_name = '存储管理', category_order = 6, display_order = 0 
WHERE function_name = 'compactions' AND is_system = 1;

-- ==============================================
-- 7. Clean up duplicate system functions (if any)
-- ==============================================
-- Keep only the latest record for each function_name
DELETE FROM system_functions 
WHERE is_system = 1 
AND id NOT IN (
    SELECT MAX(id) 
    FROM system_functions 
    WHERE is_system = 1 
    GROUP BY function_name
);

-- ==============================================
-- MIGRATION COMPLETE
-- ==============================================
-- Changes:
--   System Functions:
--     - Removed 9 unsupported PROC paths from system_functions
--     - Added 11 missing supported PROC paths
--     - Fixed colocation_group typo (was colocate_group)
--     - Reorganized categories for better UX
--     - Cleaned up duplicate records
--
--   Permissions:
--     - Removed 9 unsupported API permissions from permissions table
--     - Removed corresponding role_permissions mappings
--     - Added 9 new supported API permissions
--     - Linked new permissions to menu:system
--     - Assigned new permissions to admin role

