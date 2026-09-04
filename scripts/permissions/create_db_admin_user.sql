-- ========================================
-- 创建 StarRocks 数据库管理员用户
-- 版本: 1.0
-- 最后更新: 2026-04-01
-- ========================================

-- 1. 创建数据库管理员用户
-- ========================================
-- 根据实际情况修改：
-- - 用户名: db_admin
-- - 密码: 请修改为强密码
-- - IP限制: '%' 表示任意IP，生产环境建议改为具体IP段

DROP USER IF EXISTS 'db_admin'@'%';

CREATE USER 'db_admin'@'%' 
  IDENTIFIED BY 'DB@Admin2026!';

-- 2. 授予角色
-- ========================================
-- 授予内置的 db_admin 角色，拥有管理数据库的所有权限
GRANT db_admin TO USER 'db_admin'@'%';

-- 3. 设置默认角色
-- ========================================
SET DEFAULT ROLE db_admin TO 'db_admin'@'%';

-- 4. 验证用户权限
-- ========================================
SHOW GRANTS FOR 'db_admin'@'%';

-- ========================================
-- 输出提示信息
-- ========================================
SELECT '✅ 数据库管理员用户创建成功！' as status;
SELECT 'db_admin' as username;
SELECT 'DB@Admin2026!' as password;
SELECT '⚠️  请立即修改密码！' as warning;
SELECT 'ALTER USER ''db_admin''@''%'' IDENTIFIED BY ''Your_New_Password'';' as change_password_example;
SELECT '📝 该用户拥有管理数据库的所有权限' as permission_info;
