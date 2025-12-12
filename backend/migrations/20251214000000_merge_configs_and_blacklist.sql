-- Merge: FE configure permission + SQL blacklist permissions
-- This migration consolidates:
-- 1) api:clusters:configs (FE configure info)
-- 2) SQL blacklist menu & APIs

-- =========================================================
-- 1) FE configure permission
-- =========================================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('api:clusters:configs', '查看FE配置', 'api', 'clusters', 'configs', 'GET /api/clusters/configs');

UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:variables')
WHERE code = 'api:clusters:configs';

INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id FROM permissions WHERE code='api:clusters:configs';

-- =========================================================
-- 2) SQL Blacklist menu & APIs
-- =========================================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('menu:queries:blacklist', 'SQL黑名单', 'menu', 'queries', 'view', '查看SQL黑名单管理'),
('api:clusters:sql:blacklist', '查询SQL黑名单列表', 'api', 'clusters', 'sql:blacklist', 'GET /api/clusters/sql-blacklist'),
('api:clusters:sql:blacklist:add', '添加SQL黑名单规则', 'api', 'clusters', 'sql:blacklist:add', 'POST /api/clusters/sql-blacklist'),
('api:clusters:sql:blacklist:delete', '删除SQL黑名单规则', 'api', 'clusters', 'sql:blacklist:delete', 'DELETE /api/clusters/sql-blacklist/:id');

-- Parent mapping
UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries')
WHERE code = 'menu:queries:blacklist';

UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries:blacklist')
WHERE code IN ('api:clusters:sql:blacklist', 'api:clusters:sql:blacklist:add', 'api:clusters:sql:blacklist:delete');

-- Grant to admin
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id FROM permissions 
WHERE code IN ('menu:queries:blacklist', 'api:clusters:sql:blacklist', 'api:clusters:sql:blacklist:add', 'api:clusters:sql:blacklist:delete');

-- Grant menu:queries:blacklist to roles that already have menu:queries
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT DISTINCT rp.role_id, (SELECT id FROM permissions WHERE code = 'menu:queries:blacklist')
FROM role_permissions rp
JOIN permissions p ON rp.permission_id = p.id
WHERE p.code = 'menu:queries';

