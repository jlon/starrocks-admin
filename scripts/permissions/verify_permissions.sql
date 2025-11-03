-- ================================================================================
-- StarRocks Admin 权限验证脚本
-- ================================================================================
-- 用途: 验证 starrocks_admin 角色和监控用户的权限是否正确配置
-- 使用: 使用监控用户登录后执行此脚本
-- 版本: 1.0.0
-- ================================================================================

-- ================================================================================
-- 使用说明
-- ================================================================================
-- 
-- 1. 使用监控用户连接到StarRocks:
--    mysql -h <fe_host> -P 9030 -u starrocks_monitor -p
--
-- 2. 执行此脚本:
--    source verify_permissions.sql
--    或
--    直接复制粘贴SQL语句执行
--
-- 3. 检查输出结果:
--    - 所有标记为 ✅ 的测试应该成功
--    - 所有标记为 ❌ 的测试应该失败(权限拒绝)
--    - 如果结果不符合预期,请参考 README_PERMISSIONS.md 排查
--
-- ================================================================================

\! echo "========================================"
\! echo "StarRocks Admin 权限验证"
\! echo "========================================"
\! echo ""

-- ================================================================================
-- 第1部分: 基础信息检查
-- ================================================================================

\! echo ">>> 第1部分: 基础信息检查"
\! echo ""

\! echo "1.1 当前用户:"
SELECT USER() as current_user;

\! echo "1.2 当前激活的角色:"
SELECT CURRENT_ROLE() as current_role;

\! echo "1.3 StarRocks版本:"
SELECT VERSION() as starrocks_version;

\! echo "1.4 当前数据库:"
SELECT DATABASE() as current_database;

\! echo ""

-- ================================================================================
-- 第2部分: 角色权限检查
-- ================================================================================

\! echo ">>> 第2部分: 角色权限检查"
\! echo ""

\! echo "2.1 检查 starrocks_admin 角色的权限:"
SHOW GRANTS FOR ROLE starrocks_admin;

\! echo "2.2 检查当前用户的权限:"
SHOW GRANTS FOR CURRENT_USER();

\! echo ""

-- ================================================================================
-- 第3部分: information_schema 访问测试
-- ================================================================================

\! echo ">>> 第3部分: information_schema 访问测试"
\! echo ""

\! echo "3.1 ✅ 应该成功: 查询数据库列表"
SELECT SCHEMA_NAME 
FROM information_schema.schemata 
LIMIT 5;

\! echo "3.2 ✅ 应该成功: 查询表数量"
SELECT COUNT(*) as table_count 
FROM information_schema.tables;

\! echo "3.3 ✅ 应该成功: 查询表元数据(前5条)"
SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE, TABLE_ROWS
FROM information_schema.tables
WHERE TABLE_SCHEMA NOT IN ('information_schema', '_statistics_')
LIMIT 5;

\! echo "3.4 ✅ 应该成功: 查询分区信息"
SELECT COUNT(*) as partition_count
FROM information_schema.partitions_meta;

\! echo "3.5 ✅ 应该成功: 查询物化视图"
SELECT COUNT(*) as mv_count
FROM information_schema.materialized_views;

\! echo "3.6 ✅ 应该成功: 查询导入任务"
SELECT COUNT(*) as load_count
FROM information_schema.loads;

\! echo ""

-- ================================================================================
-- 第4部分: 审计日志访问测试
-- ================================================================================

\! echo ">>> 第4部分: 审计日志访问测试"
\! echo ""

\! echo "4.1 ✅ 应该成功: 查询审计日志数量"
SELECT COUNT(*) as audit_log_count
FROM starrocks_audit_db__.starrocks_audit_tbl__
LIMIT 1;

\! echo "4.2 ✅ 应该成功: 查询最近10条审计日志"
SELECT timestamp, user, db, state, queryType
FROM starrocks_audit_db__.starrocks_audit_tbl__
ORDER BY timestamp DESC
LIMIT 10;

\! echo "4.3 ✅ 应该成功: 计算查询延迟(P99)"
SELECT percentile_approx(queryTime, 0.99) as p99_latency_ms
FROM starrocks_audit_db__.starrocks_audit_tbl__
WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 1 HOUR)
  AND queryTime > 0
  AND state = 'EOF';

\! echo ""

-- ================================================================================
-- 第5部分: SHOW 命令测试
-- ================================================================================

\! echo ">>> 第5部分: SHOW 命令测试"
\! echo ""

\! echo "5.1 ✅ 应该成功: SHOW DATABASES"
SHOW DATABASES;

\! echo "5.2 ✅ 应该成功: SHOW TABLES FROM information_schema"
SHOW TABLES FROM information_schema;

\! echo "5.3 ✅ 应该成功: SHOW PROCESSLIST"
SHOW PROCESSLIST;

\! echo "5.4 ✅ 应该成功: SHOW FULL PROCESSLIST"
SHOW FULL PROCESSLIST;

\! echo ""

-- ================================================================================
-- 第6部分: SHOW PROC 命令测试
-- ================================================================================

\! echo ">>> 第6部分: SHOW PROC 命令测试"
\! echo ""

\! echo "6.1 ✅ 应该成功: SHOW PROC '/backends'"
SHOW PROC '/backends';

\! echo "6.2 ✅ 应该成功: SHOW PROC '/frontends'"
SHOW PROC '/frontends';

\! echo "6.3 ✅ 应该成功: SHOW PROC '/compactions' (如果有compaction任务)"
-- 注意: 如果没有运行中的compaction任务,可能返回空结果,这是正常的
SHOW PROC '/compactions';

\! echo ""

-- ================================================================================
-- 第7部分: 权限边界测试(应该失败的操作)
-- ================================================================================

\! echo ">>> 第7部分: 权限边界测试"
\! echo ""
\! echo "以下操作应该失败(Access Denied),以验证权限配置正确"
\! echo ""

-- 注意: 以下测试会产生错误,这是预期行为!
-- 如果这些操作成功了,说明权限配置有问题

\! echo "7.1 ❌ 应该失败: 创建数据库"
\! echo "执行: CREATE DATABASE test_verify_permissions_db;"
-- CREATE DATABASE test_verify_permissions_db;
\! echo "如果能创建成功,说明权限过大,请检查配置!"
\! echo ""

\! echo "7.2 ❌ 应该失败: 创建表"
\! echo "执行: CREATE TABLE information_schema.test_table (id INT);"
-- CREATE TABLE information_schema.test_table (id INT);
\! echo "如果能创建成功,说明权限过大,请检查配置!"
\! echo ""

\! echo "7.3 ❌ 应该失败: 插入数据"
\! echo "执行: INSERT INTO starrocks_audit_db__.starrocks_audit_tbl__ VALUES (...);"
-- INSERT INTO starrocks_audit_db__.starrocks_audit_tbl__ VALUES (...);
\! echo "如果能插入成功,说明权限过大,请检查配置!"
\! echo ""

\! echo "7.4 ❌ 应该失败: 删除数据"
\! echo "执行: DELETE FROM starrocks_audit_db__.starrocks_audit_tbl__ LIMIT 1;"
-- DELETE FROM starrocks_audit_db__.starrocks_audit_tbl__ LIMIT 1;
\! echo "如果能删除成功,说明权限过大,请检查配置!"
\! echo ""

\! echo "7.5 ❌ 应该失败: 更新数据"
\! echo "执行: UPDATE starrocks_audit_db__.starrocks_audit_tbl__ SET user='test' LIMIT 1;"
-- UPDATE starrocks_audit_db__.starrocks_audit_tbl__ SET user='test' LIMIT 1;
\! echo "如果能更新成功,说明权限过大,请检查配置!"
\! echo ""

\! echo "注意: 上述DDL/DML操作已注释,如需实际测试权限边界,请手动取消注释执行"
\! echo ""

-- ================================================================================
-- 第8部分: 项目关键功能测试
-- ================================================================================

\! echo ">>> 第8部分: 项目关键功能测试"
\! echo ""

\! echo "8.1 ✅ 查询表大小(Top 10)"
SELECT 
    TABLE_SCHEMA,
    TABLE_NAME,
    COALESCE(DATA_LENGTH, 0) as size_bytes,
    ROUND(COALESCE(DATA_LENGTH, 0) / 1024 / 1024 / 1024, 2) as size_gb
FROM information_schema.tables
WHERE TABLE_SCHEMA NOT IN ('information_schema', '_statistics_')
  AND DATA_LENGTH > 0
ORDER BY size_bytes DESC
LIMIT 10;

\! echo "8.2 ✅ 统计活跃用户(最近1小时)"
SELECT 
    COUNT(DISTINCT user) as active_users_1h
FROM starrocks_audit_db__.starrocks_audit_tbl__
WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 1 HOUR);

\! echo "8.3 ✅ 计算QPS(最近1小时,按分钟聚合)"
SELECT 
    DATE_FORMAT(timestamp, '%Y-%m-%d %H:%i:00') as time_bucket,
    COUNT(*) / 60.0 as qps
FROM starrocks_audit_db__.starrocks_audit_tbl__
WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 1 HOUR)
GROUP BY time_bucket
ORDER BY time_bucket DESC
LIMIT 5;

\! echo "8.4 ✅ 查询慢查询(queryTime > 1000ms, 最近1小时)"
SELECT 
    timestamp,
    user,
    db,
    queryTime as duration_ms,
    LEFT(stmt, 100) as query_preview
FROM starrocks_audit_db__.starrocks_audit_tbl__
WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 1 HOUR)
  AND queryTime > 1000
  AND state = 'EOF'
ORDER BY queryTime DESC
LIMIT 10;

\! echo "8.5 ✅ 统计BE节点信息"
-- 注意: SHOW PROC输出格式不同,这里只做基础测试
SHOW PROC '/backends';

\! echo ""

-- ================================================================================
-- 验证完成
-- ================================================================================

\! echo "========================================"
\! echo "验证完成!"
\! echo "========================================"
\! echo ""
\! echo "请检查以上输出:"
\! echo "  1. 所有标记为 ✅ 的查询应该成功返回结果"
\! echo "  2. 所有标记为 ❌ 的操作应该失败(已注释,手动测试)"
\! echo "  3. 如果结果不符合预期,请参考 README_PERMISSIONS.md"
\! echo ""
\! echo "下一步:"
\! echo "  1. 在 StarRocks Admin 项目中配置此监控用户"
\! echo "  2. 测试项目功能是否正常工作"
\! echo "  3. 定期审查权限配置和使用情况"
\! echo ""
\! echo "========================================"

