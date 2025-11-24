-- ========================================
-- StarRocks Admin - Multitenancy (Organizations)
-- ========================================
-- Created: 2025-01-27
-- Purpose: Introduce organizations and per-organization data isolation

-- 1. Organizations table
CREATE TABLE IF NOT EXISTS organizations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    is_system BOOLEAN DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_organizations_code ON organizations(code);
CREATE INDEX IF NOT EXISTS idx_organizations_is_system ON organizations(is_system);

-- 2. User-Organization mapping table (one user belongs to one organization)
CREATE TABLE IF NOT EXISTS user_organizations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_user_organizations_user_id ON user_organizations(user_id);
CREATE INDEX IF NOT EXISTS idx_user_organizations_org_id ON user_organizations(organization_id);

-- 3. Extend clusters with organization_id and enforce one active cluster per organization
ALTER TABLE clusters ADD COLUMN organization_id INTEGER NULL;
CREATE INDEX IF NOT EXISTS idx_clusters_organization_id ON clusters(organization_id);
-- Ensure only one active cluster per organization (partial unique index)
CREATE UNIQUE INDEX IF NOT EXISTS idx_clusters_org_active ON clusters(organization_id)
WHERE is_active = 1;

-- 4. Extend roles with organization_id (NULL = system role like super_admin)
ALTER TABLE roles ADD COLUMN organization_id INTEGER NULL;
CREATE INDEX IF NOT EXISTS idx_roles_organization_id ON roles(organization_id);

-- 5. (Optional) Extend users with organization_id for faster lookup
ALTER TABLE users ADD COLUMN organization_id INTEGER NULL;
CREATE INDEX IF NOT EXISTS idx_users_organization_id ON users(organization_id);

-- 6. Seed default system organization
INSERT OR IGNORE INTO organizations (code, name, description, is_system)
VALUES ('default_org', 'Default Organization', 'System default organization (built-in)', 1);

-- 7. Migrate existing data into default_org
-- 7.1 users -> user_organizations (only users not yet mapped)
INSERT OR IGNORE INTO user_organizations (user_id, organization_id)
SELECT u.id, (SELECT id FROM organizations WHERE code = 'default_org')
FROM users u;

-- 7.2 users.organization_id redundancy
UPDATE users
SET organization_id = (SELECT id FROM organizations WHERE code = 'default_org')
WHERE organization_id IS NULL;

-- 7.3 clusters.organization_id -> default_org if null
UPDATE clusters
SET organization_id = (SELECT id FROM organizations WHERE code = 'default_org')
WHERE organization_id IS NULL;

-- 7.4 roles.organization_id: system roles -> NULL, others -> default_org
UPDATE roles
SET organization_id = NULL
WHERE is_system = 1;

UPDATE roles
SET organization_id = (SELECT id FROM organizations WHERE code = 'default_org')
WHERE (organization_id IS NULL) AND (is_system = 0);

-- 8. Super Admin role (system-wide)
INSERT OR IGNORE INTO roles (code, name, description, is_system, organization_id)
VALUES ('super_admin', '超级管理员', '拥有所有权限（跨组织）', 1, NULL);

-- 9. Organization-related permissions
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description)
VALUES
('menu:organizations', '组织管理', 'menu', 'organizations', 'view', '组织管理菜单'),
('api:organizations:list', '查询组织列表', 'api', 'organizations', 'list', 'GET /api/organizations'),
('api:organizations:get', '查看组织详情', 'api', 'organizations', 'get', 'GET /api/organizations/:id'),
('api:organizations:create', '创建组织', 'api', 'organizations', 'create', 'POST /api/organizations'),
('api:organizations:update', '更新组织', 'api', 'organizations', 'update', 'PUT /api/organizations/:id'),
('api:organizations:delete', '删除组织', 'api', 'organizations', 'delete', 'DELETE /api/organizations/:id');

-- 10. Grant all permissions to super_admin (including newly added ones)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='super_admin'), p.id FROM permissions p;

-- 11. Assign super_admin role to default admin user
INSERT OR IGNORE INTO user_roles (user_id, role_id)
SELECT u.id, (SELECT id FROM roles WHERE code='super_admin')
FROM users u
WHERE u.username = 'admin'
LIMIT 1;

-- 12. Ensure existing admin role keeps full permissions (compatibility)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), p.id FROM permissions p;

-- 13. Create org_admin role for default_org (organization-scoped admin)
INSERT OR IGNORE INTO roles (code, name, description, is_system, organization_id)
SELECT 'org_admin_default_org', 'Organization Admin (Default Org)', 'Admin for default organization', 0, 
       (SELECT id FROM organizations WHERE code = 'default_org');

-- 14. Grant org_admin permissions (all except organization management)
-- Exclude both menu:system:organizations and api:organizations:* permissions
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id 
FROM roles r, permissions p
WHERE r.code = 'org_admin_default_org' 
  AND p.code NOT IN ('menu:system:organizations')
  AND p.code NOT LIKE 'api:organizations:%';

-- 15. Backfill parent menu for organization APIs (if needed)
UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:organizations')
WHERE code IN (
    'api:organizations:list',
    'api:organizations:get',
    'api:organizations:create',
    'api:organizations:update',
    'api:organizations:delete'
);

-- ========================================
-- PART 2: System Menu Hierarchy
-- ========================================
-- Purpose: Reorganize system management menus (users, roles, organizations)
--          to have parent-child hierarchy like nodes and queries menus
-- Date: 2025-11-21

-- 16. Rename old menu:system (功能卡片) to menu:system-functions to avoid conflict
-- ========================================

UPDATE permissions 
SET code = 'menu:system-functions',
    name = '功能卡片',
    resource = 'system-functions'
WHERE code = 'menu:system' AND name = '功能卡片';

-- Update API permissions parent_id from old menu:system to menu:system-functions
UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system-functions')
WHERE parent_id = (SELECT id FROM permissions WHERE code = 'menu:system' AND name = '功能卡片')
  AND code LIKE 'api:clusters:system%';

-- 17. Rename organization menu to have system: prefix
-- ========================================
-- Note: menu:system:users and menu:system:roles are created with correct names
-- in the RBAC migration file, so no rename needed here

-- Rename menu:organizations -> menu:system:organizations
UPDATE permissions 
SET code = 'menu:system:organizations',
    resource = 'system:organizations'
WHERE code = 'menu:organizations';

-- 18. Create new parent menu:system permission (系统管理)
-- ========================================

INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description)
VALUES ('menu:system', '系统管理', 'menu', 'system', 'view', '系统管理菜单（父级）');

-- 19. Set parent_id for child menus
-- ========================================

UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system')
WHERE code IN ('menu:system:users', 'menu:system:roles', 'menu:system:organizations');

-- 20. Update API permissions parent_id
-- ========================================

-- Users API permissions
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

-- Roles API permissions
UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system:roles')
WHERE code IN (
    'api:roles:list',
    'api:roles:get',
    'api:roles:create',
    'api:roles:update',
    'api:roles:delete',
    'api:roles:permissions',
    'api:roles:permissions:assign',
    'api:permissions:list',
    'api:permissions:menu'
);

-- Organizations API permissions (update parent from menu:organizations to menu:system:organizations)
UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system:organizations')
WHERE code IN (
    'api:organizations:list',
    'api:organizations:get',
    'api:organizations:create',
    'api:organizations:update',
    'api:organizations:delete',
    'api:organizations:roles',
    'api:organizations:users',
    'api:organizations:clusters'
);

-- 21. Auto-grant parent menu permissions for roles with child menus
-- ========================================
-- Purpose: Ensure that when a role has child menu permissions, 
--          it automatically gets the parent menu permission

-- Grant menu:system to roles that have any system child menu (users/roles/organizations)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT DISTINCT rp.role_id, (SELECT id FROM permissions WHERE code = 'menu:system')
FROM role_permissions rp
JOIN permissions p ON rp.permission_id = p.id
WHERE p.code IN ('menu:system:users', 'menu:system:roles', 'menu:system:organizations');

-- Grant menu:nodes to roles that have any child menu permissions (frontends/backends)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT DISTINCT rp.role_id, (SELECT id FROM permissions WHERE code = 'menu:nodes')
FROM role_permissions rp
JOIN permissions p ON rp.permission_id = p.id
WHERE p.code IN ('menu:nodes:frontends', 'menu:nodes:backends');

-- Grant menu:queries to roles that have any child menu permissions (execution/profiles/audit-logs)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT DISTINCT rp.role_id, (SELECT id FROM permissions WHERE code = 'menu:queries')
FROM role_permissions rp
JOIN permissions p ON rp.permission_id = p.id
WHERE p.code IN ('menu:queries:execution', 'menu:queries:profiles', 'menu:queries:audit-logs');