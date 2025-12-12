-- Add FE configure info API permission and parent mapping

-- 1) Permission
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('api:clusters:configs', '查看FE配置', 'api', 'clusters', 'configs', 'GET /api/clusters/configs');

-- 2) Parent menu (variables)
UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:variables')
WHERE code = 'api:clusters:configs';

-- 3) Grant admin
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id FROM permissions WHERE code='api:clusters:configs';

