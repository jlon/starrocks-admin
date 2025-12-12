-- Add SQL Diagnosis permission
-- This permission allows users to use the LLM-enhanced SQL diagnosis feature

INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('api:clusters:sql:diagnose', 'SQL诊断', 'api', 'clusters', 'sql:diagnose', 'POST /api/clusters/:cluster_id/sql/diagnose - LLM增强的SQL性能诊断');

-- Associate with Real-time Queries menu so frontend auto-selects with menu
UPDATE permissions
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:queries:execution')
WHERE code = 'api:clusters:sql:diagnose';

-- Assign to admin role
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id FROM permissions WHERE code='api:clusters:sql:diagnose';
