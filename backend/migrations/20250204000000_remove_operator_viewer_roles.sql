-- Migration: Remove operator and viewer system roles
-- Date: 2025-02-04
-- Description: Remove the operator and viewer system roles, keeping only admin

-- ==============================================
-- 1. Remove role_permissions associations for operator and viewer
-- ==============================================
DELETE FROM role_permissions
WHERE role_id IN (
    SELECT id FROM roles WHERE code IN ('operator', 'viewer')
);

-- ==============================================
-- 2. Remove user_roles associations for operator and viewer
-- ==============================================
DELETE FROM user_roles
WHERE role_id IN (
    SELECT id FROM roles WHERE code IN ('operator', 'viewer')
);

-- ==============================================
-- 3. Delete operator and viewer roles
-- ==============================================
DELETE FROM roles WHERE code IN ('operator', 'viewer');

-- ==============================================
-- 4. Verify only admin system role remains
-- ==============================================
-- This is a verification query (not executed in migration)
-- SELECT code, name, is_system FROM roles WHERE is_system = 1;
-- Expected result: Only 'admin' role should remain

