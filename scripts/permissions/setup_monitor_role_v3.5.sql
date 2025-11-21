-- ========================================
-- StarRocks Admin 监控角色配置脚本
-- 版本: 3.5 (适用于 StarRocks 3.1+)
-- 最后更新: 2025-11-21
-- ========================================

-- 1. 创建监控角色
-- ========================================
DROP ROLE IF EXISTS starrocks_admin;
CREATE ROLE starrocks_admin;

-- 2. 授予系统操作权限 (必须)
-- ========================================
-- 用于执行 SHOW PROC '/backends', SHOW PROC '/frontends' 等命令
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;

-- 3. 授予查询系统表的权限 (必须)
-- ========================================
-- 用于查询表大小、分区信息、物化视图、导入任务等
GRANT SELECT ON ALL TABLES IN DATABASE information_schema TO ROLE starrocks_admin;

-- 4. 授予查询审计日志的权限 (必须)
-- ========================================
-- 用于计算 QPS、查询延迟、活跃用户、慢查询分析等
GRANT SELECT ON ALL TABLES IN DATABASE starrocks_audit_db__ TO ROLE starrocks_admin;

-- 5. 授予访问所有数据库元数据的权限 (可选但推荐)
-- ========================================
-- 用于查询所有数据库的表列表、统计表大小等
-- 注意：这不会授予查询表数据的权限，仅元数据
GRANT USAGE ON ALL DATABASES TO ROLE starrocks_admin;

-- 6. 验证权限配置
-- ========================================
SHOW GRANTS FOR ROLE starrocks_admin;

-- ========================================
-- 输出提示信息
-- ========================================
SELECT '✅ 角色创建成功！' as status;
SELECT '📝 下一步：创建监控用户并授予角色' as next_step;
SELECT '   CREATE USER ''starrocks_monitor''@''%'' IDENTIFIED BY ''Your_Strong_Password'';' as example;
SELECT '   GRANT starrocks_admin TO USER ''starrocks_monitor''@''%'';' as example;
SELECT '   SET DEFAULT ROLE starrocks_admin TO ''starrocks_monitor''@''%'';' as example;
