-- ========================================
-- StarRocks Admin - RBAC Permission System
-- ========================================
-- Created: 2025-01-26
-- Purpose: RBAC (Role-Based Access Control) permission system tables

-- ==============================================
-- 1. Roles Table
-- ==============================================
CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code VARCHAR(50) UNIQUE NOT NULL,  -- 角色代码：admin, operator, viewer
    name VARCHAR(100) NOT NULL,        -- 角色名称：管理员、操作员、查看者
    description TEXT,                   -- 角色描述
    is_system BOOLEAN DEFAULT 0,        -- 是否为系统内置角色
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create index on role code
CREATE INDEX IF NOT EXISTS idx_roles_code ON roles(code);

-- ==============================================
-- 2. Permissions Table
-- ==============================================
CREATE TABLE IF NOT EXISTS permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code VARCHAR(100) UNIQUE NOT NULL,  -- 权限代码：menu:dashboard, api:clusters:create
    name VARCHAR(100) NOT NULL,        -- 权限名称
    type VARCHAR(20) NOT NULL,         -- 权限类型：menu, api
    resource VARCHAR(100),               -- 资源：dashboard, clusters
    action VARCHAR(50),                 -- 操作：view, create, update, delete
    parent_id INTEGER,                  -- 父权限ID（用于树形结构）
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES permissions(id) ON DELETE CASCADE
);

-- Create indexes
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

-- Create indexes
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

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_user_roles_user_id ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role_id ON user_roles(role_id);

-- ==============================================
-- 5. Insert Default Roles
-- ==============================================
INSERT OR IGNORE INTO roles (code, name, description, is_system) VALUES
('admin', '管理员', '拥有所有权限', 1),
('operator', '操作员', '除用户管理外的所有操作权限', 1),
('viewer', '查看者', '仅查看权限，无操作权限', 1);

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
-- System Management
('menu:users', '用户管理', 'menu', 'users', 'view', '查看用户管理'),
('menu:roles', '角色管理', 'menu', 'roles', 'view', '查看角色管理');

-- ==============================================
-- 7. Insert API Permissions
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- Cluster API
('api:clusters:list', '查询集群列表', 'api', 'clusters', 'list', 'GET /api/clusters'),
('api:clusters:create', '创建集群', 'api', 'clusters', 'create', 'POST /api/clusters'),
('api:clusters:get', '查看集群详情', 'api', 'clusters', 'get', 'GET /api/clusters/:id'),
('api:clusters:update', '更新集群', 'api', 'clusters', 'update', 'PUT /api/clusters/:id'),
('api:clusters:delete', '删除集群', 'api', 'clusters', 'delete', 'DELETE /api/clusters/:id'),
('api:clusters:activate', '激活集群', 'api', 'clusters', 'activate', 'PUT /api/clusters/:id/activate'),
('api:clusters:overview', '集群概览', 'api', 'clusters', 'overview', 'GET /api/clusters/overview'),
-- Backend/Frontend API
('api:clusters:backends', 'Backend节点', 'api', 'clusters', 'backends', 'GET /api/clusters/backends'),
('api:clusters:frontends', 'Frontend节点', 'api', 'clusters', 'frontends', 'GET /api/clusters/frontends'),
-- Query API
('api:clusters:queries', '查询管理', 'api', 'clusters', 'queries', 'GET /api/clusters/queries'),
('api:clusters:queries:execute', '执行查询', 'api', 'clusters', 'queries:execute', 'POST /api/clusters/queries/execute'),
('api:clusters:queries:kill', '终止查询', 'api', 'clusters', 'queries:kill', 'DELETE /api/clusters/queries/:id'),
-- Materialized Views API
('api:clusters:materialized_views', '物化视图', 'api', 'clusters', 'materialized_views', 'GET /api/clusters/materialized_views'),
('api:clusters:materialized_views:create', '创建物化视图', 'api', 'clusters', 'materialized_views:create', 'POST /api/clusters/materialized_views'),
('api:clusters:materialized_views:update', '更新物化视图', 'api', 'clusters', 'materialized_views:update', 'PUT /api/clusters/materialized_views/:name'),
('api:clusters:materialized_views:delete', '删除物化视图', 'api', 'clusters', 'materialized_views:delete', 'DELETE /api/clusters/materialized_views/:name'),
-- Sessions API
('api:clusters:sessions', '会话管理', 'api', 'clusters', 'sessions', 'GET /api/clusters/sessions'),
('api:clusters:sessions:kill', '终止会话', 'api', 'clusters', 'sessions:kill', 'DELETE /api/clusters/sessions/:id'),
-- Variables API
('api:clusters:variables', '变量管理', 'api', 'clusters', 'variables', 'GET /api/clusters/variables'),
('api:clusters:variables:update', '更新变量', 'api', 'clusters', 'variables:update', 'PUT /api/clusters/variables/:name'),
-- System API
('api:clusters:system', '功能卡片', 'api', 'clusters', 'system', 'GET /api/clusters/system'),
-- User Management API (新增)
('api:users:list', '查询用户列表', 'api', 'users', 'list', 'GET /api/users'),
('api:users:get', '查看用户详情', 'api', 'users', 'get', 'GET /api/users/:id'),
('api:users:create', '创建用户', 'api', 'users', 'create', 'POST /api/users'),
('api:users:update', '更新用户', 'api', 'users', 'update', 'PUT /api/users/:id'),
('api:users:delete', '删除用户', 'api', 'users', 'delete', 'DELETE /api/users/:id'),
-- Role Management API (新增)
('api:roles:list', '查询角色列表', 'api', 'roles', 'list', 'GET /api/roles'),
('api:roles:get', '查看角色详情', 'api', 'roles', 'get', 'GET /api/roles/:id'),
('api:roles:create', '创建角色', 'api', 'roles', 'create', 'POST /api/roles'),
('api:roles:update', '更新角色', 'api', 'roles', 'update', 'PUT /api/roles/:id'),
('api:roles:delete', '删除角色', 'api', 'roles', 'delete', 'DELETE /api/roles/:id'),
-- Permission Management API (新增)
('api:permissions:list', '查询权限列表', 'api', 'permissions', 'list', 'GET /api/permissions'),
('api:roles:permissions:get', '查看角色权限', 'api', 'roles', 'permissions:get', 'GET /api/roles/:id/permissions'),
('api:roles:permissions:update', '更新角色权限', 'api', 'roles', 'permissions:update', 'PUT /api/roles/:id/permissions'),
-- User Role Assignment API (新增)
('api:users:roles:get', '查看用户角色', 'api', 'users', 'roles:get', 'GET /api/users/:id/roles'),
('api:users:roles:assign', '分配用户角色', 'api', 'users', 'roles:assign', 'POST /api/users/:id/roles'),
('api:users:roles:remove', '移除用户角色', 'api', 'users', 'roles:remove', 'DELETE /api/users/:id/roles/:role_id');

-- ==============================================
-- 8. Assign All Permissions to Admin Role
-- ==============================================
-- Admin role gets all permissions
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT 1, id FROM permissions;  -- admin role id = 1

-- ==============================================
-- 9. Assign Permissions to Operator Role
-- ==============================================
-- Operator gets all permissions except user/role management
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT 2, id FROM permissions 
WHERE code NOT LIKE 'menu:users%' 
  AND code NOT LIKE 'menu:roles%'
  AND code NOT LIKE 'api:users:%'
  AND code NOT LIKE 'api:roles:%'
  AND code NOT LIKE 'api:permissions:%';

-- ==============================================
-- 10. Assign Permissions to Viewer Role
-- ==============================================
-- Viewer gets only read permissions (list, get, view actions)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT 3, id FROM permissions 
WHERE action IN ('list', 'get', 'view')
  AND code NOT LIKE 'menu:users%'
  AND code NOT LIKE 'menu:roles%';

-- ==============================================
-- 11. Assign Admin Role to Default Admin User
-- ==============================================
-- Assign admin role to the default admin user (user id = 1)
INSERT OR IGNORE INTO user_roles (user_id, role_id)
SELECT id, 1 FROM users WHERE username = 'admin' LIMIT 1;

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
--   - 3 system roles (admin, operator, viewer)
--   - ~60 menu and API permissions
--   - Admin role gets all permissions
--   - Operator role gets all except user/role management
--   - Viewer role gets read-only permissions
--   - Default admin user assigned admin role

