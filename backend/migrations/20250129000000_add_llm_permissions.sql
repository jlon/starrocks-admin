-- ========================================
-- StarRocks Admin - LLM Permissions Migration
-- ========================================
-- Created: 2025-01-29
-- Purpose: Add LLM management permissions for the new LLM feature

-- ==============================================
-- 1. Add LLM Menu Permission
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('menu:system:llm', 'LLM管理', 'menu', 'system:llm', 'view', '查看LLM管理');

-- ==============================================
-- 2. Add LLM API Permissions
-- ==============================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- LLM Status
('api:llm:status', 'LLM服务状态', 'api', 'llm', 'status', 'GET /api/llm/status'),
-- LLM Providers CRUD
('api:llm:providers:list', 'LLM提供商列表', 'api', 'llm', 'providers:list', 'GET /api/llm/providers'),
('api:llm:providers:get', '查看LLM提供商', 'api', 'llm', 'providers:get', 'GET /api/llm/providers/:id'),
('api:llm:providers:active', '获取活跃LLM提供商', 'api', 'llm', 'providers:active', 'GET /api/llm/providers/active'),
('api:llm:providers:create', '创建LLM提供商', 'api', 'llm', 'providers:create', 'POST /api/llm/providers'),
('api:llm:providers:update', '更新LLM提供商', 'api', 'llm', 'providers:update', 'PUT /api/llm/providers/:id'),
('api:llm:providers:delete', '删除LLM提供商', 'api', 'llm', 'providers:delete', 'DELETE /api/llm/providers/:id'),
('api:llm:providers:activate', '激活LLM提供商', 'api', 'llm', 'providers:activate', 'POST /api/llm/providers/:id/activate'),
('api:llm:providers:deactivate', '停用LLM提供商', 'api', 'llm', 'providers:deactivate', 'POST /api/llm/providers/:id/deactivate'),
('api:llm:providers:test', '测试LLM连接', 'api', 'llm', 'providers:test', 'POST /api/llm/providers/:id/test'),
-- LLM Analysis
('api:llm:analyze:root-cause', 'LLM根因分析', 'api', 'llm', 'analyze:root-cause', 'POST /api/llm/analyze/root-cause');

-- ==============================================
-- 3. Set Parent ID for LLM API Permissions
-- ==============================================
-- Associate LLM API permissions with menu:system:llm
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system:llm')
WHERE code LIKE 'api:llm:%';

-- ==============================================
-- 4. Grant LLM Permissions to Admin Role
-- ==============================================
-- Ensure admin role has all LLM permissions
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id 
FROM permissions 
WHERE code LIKE 'menu:system:llm' OR code LIKE 'api:llm:%';

-- ==============================================
-- MIGRATION COMPLETE
-- ==============================================
-- Added:
--   - 1 menu permission: menu:system:llm
--   - 11 API permissions for LLM management
--   - All permissions assigned to admin role
