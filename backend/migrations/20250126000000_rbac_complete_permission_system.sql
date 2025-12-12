-- ========================================
-- StarRocks Admin - Complete RBAC Permission System
-- ========================================
-- Created: 2025-01-26
-- Updated: 2025-02-03
-- Purpose: Complete RBAC (Role-Based Access Control) permission system with all API permissions
-- This migration consolidates all RBAC-related migrations into a single file

-- ==============================================
-- 1. Roles Table
-- ==============================================
CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code VARCHAR(50) UNIQUE NOT NULL,  -- Role code: admin
    name VARCHAR(100) NOT NULL,        -- Role name
    description TEXT,                   -- Role description
    is_system BOOLEAN DEFAULT 0,        -- System built-in role flag
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_roles_code ON roles(code);

-- ==============================================
-- 2. Permissions Table
-- ==============================================
CREATE TABLE IF NOT EXISTS permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code VARCHAR(100) UNIQUE NOT NULL,  -- Permission code: menu:dashboard, api:clusters:create
    name VARCHAR(100) NOT NULL,        -- Permission name
    type VARCHAR(20) NOT NULL,         -- Permission type: menu, api
    resource VARCHAR(100),               -- Resource: dashboard, clusters
    action VARCHAR(50),                 -- Action: view, create, update, delete
    parent_id INTEGER,                  -- Parent permission ID (for tree structure)
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES permissions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_permissions_code ON permissions(code);
CREATE INDEX IF NOT EXISTS idx_permissions_type ON permissions(type);
CREATE INDEX IF NOT EXISTS idx_permissions_parent_id ON permissions(parent_id);

-- ==============================================
-- 3. Role Permissions Table
-- ==============================================
CREATE TABLE IF NOT EXISTS role_permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    role_id INTEGER NOT NULL,
    permission_id INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    FOREIGN KEY (permission_id) REFERENCES permissions(id) ON DELETE CASCADE,
    UNIQUE(role_id, permission_id)
);

CREATE INDEX IF NOT EXISTS idx_role_permissions_role_id ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission_id ON role_permissions(permission_id);

-- ==============================================
-- 4. User Roles Table
-- ==============================================
CREATE TABLE IF NOT EXISTS user_roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    UNIQUE(user_id, role_id)
);

CREATE INDEX IF NOT EXISTS idx_user_roles_user_id ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role_id ON user_roles(role_id);

-- ==============================================
-- 5. Insert Default Roles
-- ==============================================
INSERT OR IGNORE INTO roles (code, name, description, is_system) VALUES
('admin', '管理员', '拥有所有权限', 1);

-- ==============================================
-- 6. Insert Menu Permissions
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- Dashboard
('menu:dashboard', '集群列表', 'menu', 'dashboard', 'view', '查看集群列表'),
-- Overview
('menu:overview', '集群概览', 'menu', 'overview', 'view', '查看集群概览'),
-- Nodes
('menu:nodes', '节点管理', 'menu', 'nodes', 'view', '查看节点管理'),
('menu:nodes:frontends', 'Frontend节点', 'menu', 'nodes', 'view', '查看Frontend节点'),
('menu:nodes:backends', 'Backend节点', 'menu', 'nodes', 'view', '查看Backend节点'),
-- Queries
('menu:queries', '查询管理', 'menu', 'queries', 'view', '查看查询管理'),
('menu:queries:execution', '实时查询', 'menu', 'queries', 'view', '查看实时查询'),
('menu:queries:profiles', 'Profiles', 'menu', 'queries', 'view', '查看Profiles'),
('menu:queries:audit-logs', '审计日志', 'menu', 'queries', 'view', '查看审计日志'),
-- Materialized Views
('menu:materialized-views', '物化视图', 'menu', 'materialized-views', 'view', '查看物化视图'),
-- System
('menu:system', '功能卡片', 'menu', 'system', 'view', '查看功能卡片'),
-- Sessions
('menu:sessions', '会话管理', 'menu', 'sessions', 'view', '查看会话管理'),
-- Variables
('menu:variables', '变量管理', 'menu', 'variables', 'view', '查看变量管理'),
-- System Management (using correct hierarchical names from the start)
('menu:system:users', '用户管理', 'menu', 'system:users', 'view', '查看用户管理'),
('menu:system:roles', '角色管理', 'menu', 'system:roles', 'view', '查看角色管理');

-- ==============================================
-- 7. Insert API Permissions - Core Cluster Operations
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- Cluster CRUD
('api:clusters:list', '查询集群列表', 'api', 'clusters', 'list', 'GET /api/clusters'),
('api:clusters:create', '创建集群', 'api', 'clusters', 'create', 'POST /api/clusters'),
('api:clusters:get', '查看集群详情', 'api', 'clusters', 'get', 'GET /api/clusters/:id'),
('api:clusters:update', '更新集群', 'api', 'clusters', 'update', 'PUT /api/clusters/:id'),
('api:clusters:delete', '删除集群', 'api', 'clusters', 'delete', 'DELETE /api/clusters/:id'),
('api:clusters:activate', '激活集群', 'api', 'clusters', 'activate', 'PUT /api/clusters/:id/activate'),
('api:clusters:active', '获取活跃集群', 'api', 'clusters', 'active', 'GET /api/clusters/active'),
('api:clusters:health', '集群健康检查', 'api', 'clusters', 'health', 'GET /api/clusters/:id/health'),
('api:clusters:health:post', '集群健康检查POST', 'api', 'clusters', 'health:post', 'POST /api/clusters/:id/health'),
('api:clusters:health:test', '测试集群连接', 'api', 'clusters', 'health:test', 'POST /api/clusters/health/test'),
-- Cluster Overview
('api:clusters:overview', '集群概览', 'api', 'clusters', 'overview', 'GET /api/clusters/overview'),
('api:clusters:overview:extended', '扩展集群概览', 'api', 'clusters', 'overview:extended', 'GET /api/clusters/overview/extended'),
('api:clusters:overview:health', '集群健康卡片', 'api', 'clusters', 'overview:health', 'GET /api/clusters/overview/health'),
('api:clusters:overview:performance', '性能趋势', 'api', 'clusters', 'overview:performance', 'GET /api/clusters/overview/performance'),
('api:clusters:overview:resources', '资源趋势', 'api', 'clusters', 'overview:resources', 'GET /api/clusters/overview/resources'),
('api:clusters:overview:data:stats', '数据统计', 'api', 'clusters', 'overview:data:stats', 'GET /api/clusters/overview/data-stats'),
('api:clusters:overview:capacity:prediction', '容量预测', 'api', 'clusters', 'overview:capacity:prediction', 'GET /api/clusters/overview/capacity-prediction'),
('api:clusters:overview:compaction:details', '压缩详情统计', 'api', 'clusters', 'overview:compaction:details', 'GET /api/clusters/overview/compaction-details');

-- ==============================================
-- 8. Insert API Permissions - Nodes Management
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- Backend Operations
('api:clusters:backends', 'Backend节点列表', 'api', 'clusters', 'backends', 'GET /api/clusters/backends'),
('api:clusters:backends:delete', '删除Backend节点', 'api', 'clusters', 'backends:delete', 'DELETE /api/clusters/backends/:host/:port'),
-- Frontend Operations
('api:clusters:frontends', 'Frontend节点列表', 'api', 'clusters', 'frontends', 'GET /api/clusters/frontends');

-- ==============================================
-- 9. Insert API Permissions - Query Management
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- Catalog and Database
('api:clusters:catalogs', '查询Catalog列表', 'api', 'clusters', 'catalogs', 'GET /api/clusters/catalogs'),
('api:clusters:databases', '查询数据库列表', 'api', 'clusters', 'databases', 'GET /api/clusters/databases'),
('api:clusters:tables', '查询表列表', 'api', 'clusters', 'tables', 'GET /api/clusters/tables'),
('api:clusters:catalogs:databases', '查询Catalog和数据库树', 'api', 'clusters', 'catalogs:databases', 'GET /api/clusters/catalogs-databases'),
-- Query Operations
('api:clusters:queries', '查询管理', 'api', 'clusters', 'queries', 'GET /api/clusters/queries'),
('api:clusters:queries:execute', '执行查询', 'api', 'clusters', 'queries:execute', 'POST /api/clusters/queries/execute'),
('api:clusters:queries:kill', '终止查询', 'api', 'clusters', 'queries:kill', 'DELETE /api/clusters/queries/:id'),
('api:clusters:sql:diagnose', 'SQL诊断', 'api', 'clusters', 'sql:diagnose', 'POST /api/clusters/:cluster_id/sql/diagnose'),
('api:clusters:queries:history', '查询历史记录', 'api', 'clusters', 'queries:history', 'GET /api/clusters/queries/history'),
('api:clusters:queries:profile', '查询Profile详情', 'api', 'clusters', 'queries:profile', 'GET /api/clusters/queries/:query_id/profile'),
-- Profile Operations
('api:clusters:profiles', '查询Profile列表', 'api', 'clusters', 'profiles', 'GET /api/clusters/profiles'),
('api:clusters:profiles:get', '查看Profile详情', 'api', 'clusters', 'profiles:get', 'GET /api/clusters/profiles/:query_id');

-- ==============================================
-- 10. Insert API Permissions - Materialized Views
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('api:clusters:materialized_views', '物化视图列表', 'api', 'clusters', 'materialized_views', 'GET /api/clusters/materialized_views'),
('api:clusters:materialized_views:get', '查看物化视图详情', 'api', 'clusters', 'materialized_views:get', 'GET /api/clusters/materialized_views/:mv_name'),
('api:clusters:materialized_views:create', '创建物化视图', 'api', 'clusters', 'materialized_views:create', 'POST /api/clusters/materialized_views'),
('api:clusters:materialized_views:update', '更新物化视图', 'api', 'clusters', 'materialized_views:update', 'PUT /api/clusters/materialized_views/:name'),
('api:clusters:materialized_views:delete', '删除物化视图', 'api', 'clusters', 'materialized_views:delete', 'DELETE /api/clusters/materialized_views/:name'),
('api:clusters:materialized_views:ddl', '获取物化视图DDL', 'api', 'clusters', 'materialized_views:ddl', 'GET /api/clusters/materialized_views/:mv_name/ddl'),
('api:clusters:materialized_views:refresh', '刷新物化视图', 'api', 'clusters', 'materialized_views:refresh', 'POST /api/clusters/materialized_views/:mv_name/refresh'),
('api:clusters:materialized_views:cancel', '取消刷新物化视图', 'api', 'clusters', 'materialized_views:cancel', 'POST /api/clusters/materialized_views/:mv_name/cancel'),
('api:clusters:materialized_views:alter', '修改物化视图', 'api', 'clusters', 'materialized_views:alter', 'PUT /api/clusters/materialized_views/:mv_name');

-- ==============================================
-- 11. Insert API Permissions - Sessions & Variables
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- Sessions
('api:clusters:sessions', '会话管理', 'api', 'clusters', 'sessions', 'GET /api/clusters/sessions'),
('api:clusters:sessions:kill', '终止会话', 'api', 'clusters', 'sessions:kill', 'DELETE /api/clusters/sessions/:id'),
-- Variables
('api:clusters:variables', '变量管理', 'api', 'clusters', 'variables', 'GET /api/clusters/variables'),
('api:clusters:variables:update', '更新变量', 'api', 'clusters', 'variables:update', 'PUT /api/clusters/variables/:name');

-- ==============================================
-- 12. Insert API Permissions - System Management
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- System Info
('api:clusters:system', '功能卡片', 'api', 'clusters', 'system', 'GET /api/clusters/system'),
('api:clusters:system:runtime_info', '查询运行时信息', 'api', 'clusters', 'system:runtime_info', 'GET /api/clusters/system/runtime_info'),
-- System Function Details (25 system functions)
('api:clusters:system:brokers', '查询Broker节点', 'api', 'clusters', 'system:brokers', 'GET /api/clusters/system/brokers'),
('api:clusters:system:frontends', '查询Frontend节点', 'api', 'clusters', 'system:frontends', 'GET /api/clusters/system/frontends'),
('api:clusters:system:routine_loads', '查询Routine Load任务', 'api', 'clusters', 'system:routine_loads', 'GET /api/clusters/system/routine_loads'),
('api:clusters:system:catalog', '查询目录', 'api', 'clusters', 'system:catalog', 'GET /api/clusters/system/catalog'),
('api:clusters:system:colocation_group', '查询Colocation Group', 'api', 'clusters', 'system:colocation_group', 'GET /api/clusters/system/colocation_group'),
('api:clusters:system:cluster_balance', '查询集群平衡', 'api', 'clusters', 'system:cluster_balance', 'GET /api/clusters/system/cluster_balance'),
('api:clusters:system:load_error_hub', '查询加载错误信息', 'api', 'clusters', 'system:load_error_hub', 'GET /api/clusters/system/load_error_hub'),
('api:clusters:system:meta_recovery', '查询元数据恢复', 'api', 'clusters', 'system:meta_recovery', 'GET /api/clusters/system/meta_recovery'),
('api:clusters:system:global_current_queries', '查询全局当前查询', 'api', 'clusters', 'system:global_current_queries', 'GET /api/clusters/system/global_current_queries'),
('api:clusters:system:tasks', '查询系统任务', 'api', 'clusters', 'system:tasks', 'GET /api/clusters/system/tasks'),
('api:clusters:system:compute_nodes', '查询计算节点', 'api', 'clusters', 'system:compute_nodes', 'GET /api/clusters/system/compute_nodes'),
('api:clusters:system:statistic', '查询统计信息', 'api', 'clusters', 'system:statistic', 'GET /api/clusters/system/statistic'),
('api:clusters:system:jobs', '查询后台任务', 'api', 'clusters', 'system:jobs', 'GET /api/clusters/system/jobs'),
('api:clusters:system:warehouses', '查询仓库', 'api', 'clusters', 'system:warehouses', 'GET /api/clusters/system/warehouses'),
('api:clusters:system:resources', '查询资源', 'api', 'clusters', 'system:resources', 'GET /api/clusters/system/resources'),
('api:clusters:system:transactions', '查询事务', 'api', 'clusters', 'system:transactions', 'GET /api/clusters/system/transactions'),
('api:clusters:system:backends', '查询Backend节点', 'api', 'clusters', 'system:backends', 'GET /api/clusters/system/backends'),
('api:clusters:system:current_queries', '查询当前查询', 'api', 'clusters', 'system:current_queries', 'GET /api/clusters/system/current_queries'),
('api:clusters:system:stream_loads', '查询Stream Load任务', 'api', 'clusters', 'system:stream_loads', 'GET /api/clusters/system/stream_loads'),
('api:clusters:system:replications', '查询复制状态', 'api', 'clusters', 'system:replications', 'GET /api/clusters/system/replications'),
('api:clusters:system:dbs', '查询数据库', 'api', 'clusters', 'system:dbs', 'GET /api/clusters/system/dbs'),
('api:clusters:system:current_backend_instances', '查询当前Backend实例', 'api', 'clusters', 'system:current_backend_instances', 'GET /api/clusters/system/current_backend_instances'),
('api:clusters:system:historical_nodes', '查询历史节点', 'api', 'clusters', 'system:historical_nodes', 'GET /api/clusters/system/historical_nodes'),
('api:clusters:system:compactions', '查询压缩任务', 'api', 'clusters', 'system:compactions', 'GET /api/clusters/system/compactions'),
-- Additional supported PROC paths
('api:clusters:system:monitor', '查询监控信息', 'api', 'clusters', 'system:monitor', 'GET /api/clusters/system/monitor'),
('api:clusters:system:cluster_balance', '查询集群均衡', 'api', 'clusters', 'system:cluster_balance', 'GET /api/clusters/system/cluster_balance'),
('api:clusters:system:meta_recovery', '查询元数据恢复', 'api', 'clusters', 'system:meta_recovery', 'GET /api/clusters/system/meta_recovery'),
('api:clusters:system:colocation_group', '查询Colocation Group', 'api', 'clusters', 'system:colocation_group', 'GET /api/clusters/system/colocation_group'),
-- System Functions
('api:clusters:system:functions', '查询系统函数列表', 'api', 'clusters', 'system:functions', 'GET /api/clusters/system-functions'),
('api:clusters:system:functions:create', '创建系统函数', 'api', 'clusters', 'system:functions:create', 'POST /api/clusters/system-functions'),
('api:clusters:system:functions:update', '更新系统函数', 'api', 'clusters', 'system:functions:update', 'PUT /api/clusters/system-functions/:function_id'),
('api:clusters:system:functions:delete', '删除系统函数', 'api', 'clusters', 'system:functions:delete', 'DELETE /api/clusters/system-functions/:function_id'),
('api:clusters:system:functions:orders', '更新系统函数顺序', 'api', 'clusters', 'system:functions:orders', 'PUT /api/clusters/system-functions/orders'),
('api:clusters:system:functions:execute', '执行系统函数', 'api', 'clusters', 'system:functions:execute', 'POST /api/clusters/system-functions/:function_id/execute'),
('api:clusters:system:functions:favorite', '切换系统函数收藏', 'api', 'clusters', 'system:functions:favorite', 'PUT /api/clusters/system-functions/:function_id/favorite'),
-- System Functions (non-cluster specific)
('api:system:functions:access-time', '更新系统函数访问时间', 'api', 'system', 'functions:access-time', 'PUT /api/system-functions/:function_name/access-time'),
('api:system:functions:category:delete', '删除系统函数分类', 'api', 'system', 'functions:category:delete', 'DELETE /api/system-functions/category/:category_name');

-- ==============================================
-- 13. Insert API Permissions - RBAC Management
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- User Management
('api:users:list', '查询用户列表', 'api', 'users', 'list', 'GET /api/users'),
('api:users:get', '查看用户详情', 'api', 'users', 'get', 'GET /api/users/:id'),
('api:users:create', '创建用户', 'api', 'users', 'create', 'POST /api/users'),
('api:users:update', '更新用户', 'api', 'users', 'update', 'PUT /api/users/:id'),
('api:users:delete', '删除用户', 'api', 'users', 'delete', 'DELETE /api/users/:id'),
-- Role Management
('api:roles:list', '查询角色列表', 'api', 'roles', 'list', 'GET /api/roles'),
('api:roles:get', '查看角色详情', 'api', 'roles', 'get', 'GET /api/roles/:id'),
('api:roles:create', '创建角色', 'api', 'roles', 'create', 'POST /api/roles'),
('api:roles:update', '更新角色', 'api', 'roles', 'update', 'PUT /api/roles/:id'),
('api:roles:delete', '删除角色', 'api', 'roles', 'delete', 'DELETE /api/roles/:id'),
-- Permission Management
('api:permissions:list', '查询权限列表', 'api', 'permissions', 'list', 'GET /api/permissions'),
('api:permissions:menu', '查询菜单权限', 'api', 'permissions', 'menu', 'GET /api/permissions/menu'),
('api:permissions:api', '查询API权限', 'api', 'permissions', 'api', 'GET /api/permissions/api'),
('api:permissions:tree', '查询权限树', 'api', 'permissions', 'tree', 'GET /api/permissions/tree'),
-- Role Permissions
('api:roles:permissions:get', '查看角色权限', 'api', 'roles', 'permissions:get', 'GET /api/roles/:id/permissions'),
('api:roles:permissions:update', '更新角色权限', 'api', 'roles', 'permissions:update', 'PUT /api/roles/:id/permissions'),
-- User Role Assignment
('api:users:roles:get', '查看用户角色', 'api', 'users', 'roles:get', 'GET /api/users/:id/roles'),
('api:users:roles:assign', '分配用户角色', 'api', 'users', 'roles:assign', 'POST /api/users/:id/roles'),
('api:users:roles:remove', '移除用户角色', 'api', 'users', 'roles:remove', 'DELETE /api/users/:id/roles/:role_id');

-- ==============================================
-- 14. Insert API Permissions - Auth
-- ==============================================
-- Note: These are basic permissions that all logged-in users need
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('api:auth:me', '获取当前用户信息', 'api', 'auth', 'me', 'GET /api/auth/me'),
('api:auth:me:update', '更新当前用户信息', 'api', 'auth', 'me:update', 'PUT /api/auth/me'),
('api:auth:permissions', '获取当前用户权限', 'api', 'auth', 'permissions', 'GET /api/auth/permissions');

-- ==============================================
-- 15. Assign All Permissions to Admin Role
-- ==============================================
-- Admin role gets ALL permissions (both menu and api)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id FROM permissions;

-- ==============================================
-- 16. Assign Admin Role to Default Admin User
-- ==============================================
-- Assign admin role to the default admin user
INSERT OR IGNORE INTO user_roles (user_id, role_id)
SELECT u.id, (SELECT id FROM roles WHERE code='admin') 
FROM users u 
WHERE u.username = 'admin' 
LIMIT 1;

-- ========================================
-- MIGRATION COMPLETE
-- ========================================
-- Tables Summary:
--   1. roles                         - Role definitions
--   2. permissions                   - Permission definitions
--   3. role_permissions             - Role-Permission mappings
--   4. user_roles                   - User-Role mappings
--
-- Default Data:
--   - 1 system role (admin)
--   - 14 menu permissions
--   - 80 API permissions (76 + 4 new: health:post, health:test, auth:permissions, sql:diagnose)
--   - Admin role gets ALL permissions
--   - Default admin user assigned admin role
--
-- Permission Coverage:
--   ✓ Cluster CRUD & Health Check (including POST methods and test connection)
--   ✓ Cluster Overview (8 endpoints)
--   ✓ Backend/Frontend Management
--   ✓ Query Management (Catalogs, Databases, Tables)
--   ✓ Query Execution & History
--   ✓ Profile Management
--   ✓ Materialized Views (9 operations)
--   ✓ Sessions & Variables
--   ✓ System Functions (12 operations)
--   ✓ RBAC Management (Users, Roles, Permissions)
--   ✓ Auth (Me endpoints + Current User Permissions)
--
-- ==============================================
-- Menu-API Permission Mapping Reference
-- ==============================================
-- This section documents the relationship between menu permissions
-- and their corresponding API permissions for proper RBAC enforcement.
-- The frontend automatically associates API permissions when a menu
-- is selected during role creation/editing.
--
-- 【集群列表】menu:dashboard
--   ├─ api:clusters:list              (GET /api/clusters)
--   ├─ api:clusters:create            (POST /api/clusters)
--   ├─ api:clusters:get               (GET /api/clusters/:id)
--   ├─ api:clusters:update            (PUT /api/clusters/:id)
--   ├─ api:clusters:delete            (DELETE /api/clusters/:id)
--   ├─ api:clusters:activate          (PUT /api/clusters/:id/activate)
--   ├─ api:clusters:active            (GET /api/clusters/active)
--   └─ api:clusters:health            (GET /api/clusters/:id/health)
--
-- 【集群概览】menu:overview
--   ├─ api:clusters:overview                   (GET /api/clusters/overview)
--   ├─ api:clusters:overview:extended          (GET /api/clusters/overview/extended)
--   ├─ api:clusters:overview:health            (GET /api/clusters/overview/health)
--   ├─ api:clusters:overview:performance       (GET /api/clusters/overview/performance)
--   ├─ api:clusters:overview:resources         (GET /api/clusters/overview/resources)
--   ├─ api:clusters:overview:data:stats        (GET /api/clusters/overview/data-stats)
--   ├─ api:clusters:overview:capacity:prediction  (GET /api/clusters/overview/capacity-prediction)
--   └─ api:clusters:overview:compaction:details   (GET /api/clusters/overview/compaction-details)
--
-- 【节点管理】menu:nodes (parent)
--   No direct APIs (parent menu only)
--
-- 【Frontend节点】menu:nodes:frontends
--   └─ api:clusters:frontends         (GET /api/clusters/frontends)
--
-- 【Backend节点】menu:nodes:backends
--   ├─ api:clusters:backends          (GET /api/clusters/backends)
--   └─ api:clusters:backends:delete   (DELETE /api/clusters/backends/:host/:port)
--
-- 【查询管理】menu:queries (parent)
--   No direct APIs (parent menu only)
--
-- 【实时查询】menu:queries:execution
--   ├─ api:clusters:catalogs          (GET /api/clusters/catalogs)
--   ├─ api:clusters:databases         (GET /api/clusters/databases)
--   ├─ api:clusters:tables            (GET /api/clusters/tables)
--   ├─ api:clusters:catalogs:databases (GET /api/clusters/catalogs-databases)
--   ├─ api:clusters:queries           (GET /api/clusters/queries)
--   ├─ api:clusters:queries:execute   (POST /api/clusters/queries/execute)
--   ├─ api:clusters:queries:kill      (DELETE /api/clusters/queries/:id)
--   └─ api:clusters:sql:diagnose      (POST /api/clusters/:cluster_id/sql/diagnose)
--
-- 【Profiles】menu:queries:profiles
--   ├─ api:clusters:profiles          (GET /api/clusters/profiles)
--   ├─ api:clusters:profiles:get      (GET /api/clusters/profiles/:query_id)
--   └─ api:clusters:queries:profile   (GET /api/clusters/queries/:query_id/profile)
--
-- 【审计日志】menu:queries:audit-logs
--   └─ api:clusters:queries:history   (GET /api/clusters/queries/history)
--
-- 【物化视图】menu:materialized-views
--   ├─ api:clusters:materialized_views         (GET /api/clusters/materialized_views)
--   ├─ api:clusters:materialized_views:get     (GET /api/clusters/materialized_views/:mv_name)
--   ├─ api:clusters:materialized_views:create  (POST /api/clusters/materialized_views)
--   ├─ api:clusters:materialized_views:update  (PUT /api/clusters/materialized_views/:name)
--   ├─ api:clusters:materialized_views:delete  (DELETE /api/clusters/materialized_views/:name)
--   ├─ api:clusters:materialized_views:ddl     (GET /api/clusters/materialized_views/:mv_name/ddl)
--   ├─ api:clusters:materialized_views:refresh (POST /api/clusters/materialized_views/:mv_name/refresh)
--   ├─ api:clusters:materialized_views:cancel  (POST /api/clusters/materialized_views/:mv_name/cancel)
--   └─ api:clusters:materialized_views:alter   (PUT /api/clusters/materialized_views/:mv_name)
--
-- 【功能卡片】menu:system
--   ├─ api:clusters:system                      (GET /api/clusters/system)
--   ├─ api:clusters:system:runtime_info         (GET /api/clusters/system/runtime_info)
--   ├─ api:clusters:system:function             (GET /api/clusters/system/:function_name)
--   ├─ api:clusters:system:functions            (GET /api/clusters/system-functions)
--   ├─ api:clusters:system:functions:create     (POST /api/clusters/system-functions)
--   ├─ api:clusters:system:functions:update     (PUT /api/clusters/system-functions/:function_id)
--   ├─ api:clusters:system:functions:delete     (DELETE /api/clusters/system-functions/:function_id)
--   ├─ api:clusters:system:functions:orders     (PUT /api/clusters/system-functions/orders)
--   ├─ api:clusters:system:functions:execute    (POST /api/clusters/system-functions/:function_id/execute)
--   ├─ api:clusters:system:functions:favorite   (PUT /api/clusters/system-functions/:function_id/favorite)
--   ├─ api:system:functions:access-time         (PUT /api/system-functions/:function_name/access-time)
--   └─ api:system:functions:category:delete     (DELETE /api/system-functions/category/:category_name)
--
-- 【会话管理】menu:sessions
--   ├─ api:clusters:sessions          (GET /api/clusters/sessions)
--   └─ api:clusters:sessions:kill     (DELETE /api/clusters/sessions/:id)
--
-- 【变量管理】menu:variables
--   ├─ api:clusters:variables         (GET /api/clusters/variables)
--   └─ api:clusters:variables:update  (PUT /api/clusters/variables/:name)
--
-- 【用户管理】menu:users
--   ├─ api:users:list                 (GET /api/users)
--   ├─ api:users:get                  (GET /api/users/:id)
--   ├─ api:users:create               (POST /api/users)
--   ├─ api:users:update               (PUT /api/users/:id)
--   ├─ api:users:delete               (DELETE /api/users/:id)
--   ├─ api:users:roles:get            (GET /api/users/:id/roles)
--   ├─ api:users:roles:assign         (POST /api/users/:id/roles)
--   └─ api:users:roles:remove         (DELETE /api/users/:id/roles/:role_id)
--
-- 【角色管理】menu:roles
--   ├─ api:roles:list                 (GET /api/roles)
--   ├─ api:roles:get                  (GET /api/roles/:id)
--   ├─ api:roles:create               (POST /api/roles)
--   ├─ api:roles:update               (PUT /api/roles/:id)
--   ├─ api:roles:delete               (DELETE /api/roles/:id)
--   ├─ api:permissions:list           (GET /api/permissions)
--   ├─ api:permissions:menu           (GET /api/permissions/menu)
--   ├─ api:permissions:api            (GET /api/permissions/api)
--   ├─ api:permissions:tree           (GET /api/permissions/tree)
--   ├─ api:roles:permissions:get      (GET /api/roles/:id/permissions)
--   └─ api:roles:permissions:update   (PUT /api/roles/:id/permissions)
--
-- 【通用权限】(不属于特定菜单)
-- 所有登录用户的基础权限，用于获取和更新个人信息
--   ├─ api:auth:me                    (GET /api/auth/me) - 查看个人资料
--   ├─ api:auth:me:update             (PUT /api/auth/me) - 修改头像、邮箱等
--   └─ api:auth:permissions           (GET /api/auth/permissions) - 获取权限列表（前端必需）
--
-- ==============================================
-- Frontend Auto-Association Logic
-- ==============================================
-- When creating or editing a role in the frontend, the role form dialog
-- (role-form-dialog.component.ts) automatically associates API permissions
-- with menu permissions using the following algorithm:
--
-- 1. Build mapping during initialization (buildApiAssociations):
--    - Extract path from permission code (e.g., "menu:nodes:backends" -> "nodes:backends")
--    - Match API paths to menu paths using multiple scoring rules:
--      a) Exact match or prefix match: score = 100 + path length
--      b) All segments match: score = 80 + path length
--      c) First segment match: score = 70 + first segment length
--      d) Last segment match: score = 60 + last segment length
--      e) Contains last segment: score = 50 + last segment length
--    - Store bidirectional mapping: menuToApis and apiToMenus
--
-- 2. On submit (submit method):
--    - Collect selected menu IDs from the form
--    - For each selected menu, lookup associated APIs from menuToApis map
--    - Merge menu IDs + API IDs into final permissionIds array
--    - Send to backend for persistence
--
-- This ensures that when a user selects a menu permission, all related
-- API permissions are automatically granted, maintaining consistency
-- between frontend navigation access and backend API access.
--
-- ==============================================
-- Data Cleanup and Fixes (For Existing Databases)
-- ==============================================
-- The following section handles cleanup and fixes for databases
-- that were created with earlier versions of this migration.
-- All operations are idempotent and safe to run multiple times.

-- ==============================================
-- 17. Remove Operator and Viewer Roles (if they exist)
-- ==============================================
-- Remove role_permissions associations for operator and viewer
DELETE FROM role_permissions
WHERE role_id IN (
    SELECT id FROM roles WHERE code IN ('operator', 'viewer')
);

-- Remove user_roles associations for operator and viewer
DELETE FROM user_roles
WHERE role_id IN (
    SELECT id FROM roles WHERE code IN ('operator', 'viewer')
);

-- Delete operator and viewer roles
DELETE FROM roles WHERE code IN ('operator', 'viewer');

-- ==============================================
-- 18. Clean Up - Removed (No longer needed in unified migration)
-- ==============================================
-- Note: In unified migration, permissions are created correctly from the start
-- No cleanup of non-existent permissions is needed

-- ==============================================
-- 19. Set Parent_ID for All Unmatched API Permissions
-- ==============================================
-- The frontend auto-matching algorithm uses path segment matching to associate
-- API permissions with menu permissions. However, some APIs cannot be automatically
-- matched due to path differences. We explicitly set parent_id for these cases.

-- 19.1 Dashboard Menu - Cluster CRUD Operations
-- menu:dashboard (path: "dashboard") cannot match api:clusters:* (path: "clusters:*")
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:dashboard')
WHERE code IN (
    'api:clusters:list',
    'api:clusters:create',
    'api:clusters:get',
    'api:clusters:update',
    'api:clusters:delete',
    'api:clusters:activate',
    'api:clusters:active',
    'api:clusters:health',
    'api:clusters:health:post',
    'api:clusters:health:test'
);

-- 19.2 Queries Execution Menu - Catalog/Database/Table APIs
-- menu:queries:execution (path: "queries:execution") cannot match api:clusters:catalogs/databases/tables
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries:execution')
WHERE code IN (
    'api:clusters:catalogs',
    'api:clusters:databases',
    'api:clusters:tables',
    'api:clusters:catalogs:databases',
    'api:clusters:sql:diagnose'
);

-- 19.3 Audit Logs Menu - Query History API
-- menu:queries:audit-logs (path: "queries:audit-logs") has weak match with api:clusters:queries:history
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries:audit-logs')
WHERE code = 'api:clusters:queries:history';

-- 19.4 Materialized Views Menu - Underscore vs Hyphen Issue
-- menu:materialized-views (hyphen) vs api:clusters:materialized_views (underscore)
-- Explicitly set parent_id to avoid potential matching issues
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:materialized-views')
WHERE code LIKE 'api:clusters:materialized_views%';

-- 19.5 Roles Menu - Permissions API
-- menu:system:roles (path: "system:roles") cannot match api:permissions:* (path: "permissions:*")
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system:roles')
WHERE code IN (
    'api:permissions:list',
    'api:permissions:menu',
    'api:permissions:api',
    'api:permissions:tree'
);

-- 19.6 Low-Score Matches - Set parent_id for Stability
-- These APIs can match via path algorithm but with low scores (<60).
-- Setting parent_id explicitly ensures correct association and stability.

-- 19.6.1 Queries Execution - Query Operation APIs
-- These should belong to menu:queries:execution, not parent menu:queries
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries:execution')
WHERE code IN (
    'api:clusters:queries',           -- Score: 67 on parent, should be on execution
    'api:clusters:queries:execute',   -- Score: 57
    'api:clusters:queries:kill'       -- Score: 57
);

-- 19.6.2 Profiles Menu - Profile Detail API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries:profiles')
WHERE code IN (
    'api:clusters:profiles:get',      -- Score: 58
    'api:clusters:queries:profile'    -- Score: 57, should be here not parent
);

-- 19.6.3 Overview Menu - Extended Overview APIs
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:overview')
WHERE code IN (
    'api:clusters:overview:extended',           -- Score: 58
    'api:clusters:overview:health',             -- Score: 58
    'api:clusters:overview:performance',        -- Score: 58
    'api:clusters:overview:resources',          -- Score: 58
    'api:clusters:overview:data:stats',         -- Score: 58
    'api:clusters:overview:capacity:prediction',-- Score: 58
    'api:clusters:overview:compaction:details'  -- Score: 58
);

-- 19.6.4 System Menu - System Function Management APIs
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system')
WHERE code IN (
    'api:clusters:system:function',          -- Score: 56
    'api:clusters:system:functions',         -- Score: 56
    'api:clusters:system:functions:create',  -- Score: 56
    'api:clusters:system:functions:update',  -- Score: 56
    'api:clusters:system:functions:delete',  -- Score: 56
    'api:clusters:system:functions:orders',  -- Score: 56
    'api:clusters:system:functions:execute', -- Score: 56
    'api:clusters:system:functions:favorite',-- Score: 56
    'api:clusters:system:runtime_info'       -- Score: 56
);

-- 19.6.5 Sessions Menu - Session Kill API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:sessions')
WHERE code = 'api:clusters:sessions:kill';  -- Score: 58

-- 19.6.6 Variables Menu - Variable Update API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:variables')
WHERE code = 'api:clusters:variables:update';  -- Score: 59

-- 19.6.7 Backend Nodes Menu - Backend Delete API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:nodes:backends')
WHERE code = 'api:clusters:backends:delete';  -- Score: 58

-- 19.7 Core List/Query APIs - Set parent_id for Base APIs
-- These are the primary list/query APIs that were previously relying on high-score matching.
-- Setting parent_id explicitly ensures 100% accuracy and prevents any potential issues.

-- 19.7.1 Backend Nodes Menu - Backend List API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:nodes:backends')
WHERE code = 'api:clusters:backends';

-- 19.7.2 Frontend Nodes Menu - Frontend List API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:nodes:frontends')
WHERE code = 'api:clusters:frontends';

-- 19.7.3 Sessions Menu - Session List API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:sessions')
WHERE code = 'api:clusters:sessions';

-- 19.7.4 Variables Menu - Variable List API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:variables')
WHERE code = 'api:clusters:variables';

-- 19.7.5 Overview Menu - Base Overview API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:overview')
WHERE code = 'api:clusters:overview';

-- 19.7.6 Profiles Menu - Profile List API
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries:profiles')
WHERE code = 'api:clusters:profiles';

-- 19.7.7 System Menu - All System APIs (based on official PROC paths)
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system')
WHERE code IN (
    'api:clusters:system',                    -- System info list
    'api:clusters:system:runtime_info',       -- Runtime info
    -- Official PROC Paths (25 supported)
    'api:clusters:system:brokers',
    'api:clusters:system:frontends',
    'api:clusters:system:routine_loads',
    'api:clusters:system:catalog',
    'api:clusters:system:colocation_group',   -- Note: correct spelling (not colocate_group)
    'api:clusters:system:cluster_balance',
    'api:clusters:system:load_error_hub',
    'api:clusters:system:meta_recovery',
    'api:clusters:system:global_current_queries',
    'api:clusters:system:tasks',
    'api:clusters:system:compute_nodes',
    'api:clusters:system:statistic',
    'api:clusters:system:jobs',
    'api:clusters:system:warehouses',
    'api:clusters:system:resources',
    'api:clusters:system:monitor',
    'api:clusters:system:transactions',
    'api:clusters:system:backends',
    'api:clusters:system:current_queries',
    'api:clusters:system:stream_loads',
    'api:clusters:system:replications',
    'api:clusters:system:dbs',
    'api:clusters:system:current_backend_instances',
    'api:clusters:system:historical_nodes',
    'api:clusters:system:compactions',
    -- System Functions Management
    'api:clusters:system:functions',
    'api:clusters:system:functions:create',
    'api:clusters:system:functions:update',
    'api:clusters:system:functions:delete',
    'api:clusters:system:functions:orders',
    'api:clusters:system:functions:execute',
    'api:clusters:system:functions:favorite',
    'api:system:functions:access-time',       -- Update access time
    'api:system:functions:category:delete'    -- Delete category
);

-- 19.7.8 Roles Menu - All Role Management APIs
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system:roles')
WHERE code IN (
    'api:roles:list',
    'api:roles:get',
    'api:roles:create',
    'api:roles:update',
    'api:roles:delete',
    'api:roles:permissions',
    'api:roles:permissions:assign'
);

-- 19.7.9 Users Menu - All User Management APIs
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system:users')
WHERE code IN (
    'api:users:list',
    'api:users:get',
    'api:users:create',
    'api:users:update',
    'api:users:delete',
    'api:users:roles',
    'api:users:roles:assign',
    'api:users:roles:remove'
);

-- ==============================================
-- 20. Ensure Admin Has ALL Permissions
-- ==============================================
-- This ensures admin role has all permissions, including any that
-- might have been missed during initial migration or added later
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), p.id 
FROM permissions p
WHERE NOT EXISTS (
    SELECT 1 FROM role_permissions rp 
    WHERE rp.role_id = (SELECT id FROM roles WHERE code='admin') 
    AND rp.permission_id = p.id
);

--

