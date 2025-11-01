-- ==============================================
-- Migration: Add /api/clusters/active permission
-- ==============================================

INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description)
VALUES ('api:clusters:active', '获取活跃集群', 'api', 'clusters', 'active', 'GET /api/clusters/active');

-- Grant to admin role
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON p.code = 'api:clusters:active'
WHERE r.code = 'admin';

-- Grant to operator role (read access)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON p.code = 'api:clusters:active'
WHERE r.code = 'operator';

-- Grant to viewer role (read-only access)
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON p.code = 'api:clusters:active'
WHERE r.code = 'viewer';

