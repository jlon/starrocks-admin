-- ========================================
-- 创建 StarRocks 监控用户
-- 版本: 3.5
-- 最后更新: 2025-11-21
-- ========================================

-- 前置检查：确保 starrocks_admin 角色已创建
-- 如果未创建，请先执行 setup_monitor_role_v3.5.sql

-- 1. 创建监控用户
-- ========================================
-- 根据实际情况修改：
-- - 用户名: starrocks_monitor
-- - 密码: 请修改为强密码
-- - IP限制: '%' 表示任意IP，生产环境建议改为具体IP段

DROP USER IF EXISTS 'starrocks_monitor'@'%';

CREATE USER 'starrocks_monitor'@'%' 
  IDENTIFIED BY 'Monitor@StarRocks2024!';

-- 2. 授予角色
-- ========================================
GRANT starrocks_admin TO USER 'starrocks_monitor'@'%';

-- 3. 设置默认角色
-- ========================================
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';

-- 4. 验证用户权限
-- ========================================
SHOW GRANTS FOR 'starrocks_monitor'@'%';

-- ========================================
-- 输出提示信息
-- ========================================
SELECT '✅ 监控用户创建成功！' as status;
SELECT 'starrocks_monitor' as username;
SELECT 'Monitor@StarRocks2024!' as password;
SELECT '⚠️  请立即修改密码！' as warning;
SELECT 'ALTER USER ''starrocks_monitor''@''%'' IDENTIFIED BY ''Your_New_Password'';' as change_password_example;
