-- ================================================================================
-- StarRocks Admin Role 权限配置脚本
-- ================================================================================
-- 用途: 为 starrocks-admin 监控平台创建只读监控角色
-- 版本: 适用于 StarRocks 3.0+
-- 文档: https://docs.starrocks.io/zh/docs/3.5/administration/user_privs/authorization/
-- 
-- 使用方法:
--   1. 使用具有 user_admin 权限的账号连接到 StarRocks
--   2. 执行本脚本创建角色
--   3. 创建监控用户并授予角色(见文末示例)
--   4. 执行验证SQL确认权限生效
--
-- 最后更新: 2025-11-03
-- ================================================================================

-- ================================================================================
-- 方案选择指南
-- ================================================================================
-- 
-- 本脚本提供三种方案,请根据实际情况选择:
--
-- 【推荐】方案A - 精确权限(最小权限原则)
--   优点: 权限最小化,安全性最高,只授予必要的监控权限
--   缺点: 语法较新,需要StarRocks 3.0+版本支持
--   适用: 生产环境,安全要求高的场景
--
-- 方案B - 兼容性权限(向后兼容)
--   优点: 兼容性好,适用于不同版本的StarRocks
--   缺点: 使用 *.* 语法,权限范围稍大
--   适用: 版本较旧的StarRocks,或方案A语法不支持时
--
-- 方案C - 内置角色(快速部署)
--   优点: 一行代码完成,部署简单快速
--   缺点: 使用db_admin包含DDL权限,不符合最小权限原则
--   适用: 测试环境,快速验证功能,非生产环境
--
-- ================================================================================


-- ================================================================================
-- 方案A: 精确权限配置(推荐)
-- ================================================================================
-- 
-- 此方案严格遵循最小权限原则,只授予监控所需的精确权限
-- 

-- 1. 创建 starrocks_admin 角色
CREATE ROLE IF NOT EXISTS starrocks_admin 
COMMENT 'StarRocks Admin监控平台只读角色 - 仅用于集群监控和元数据查询';

-- 2. 授予 information_schema 数据库的 SELECT 权限
-- 用途: 查询表元数据、数据库列表、分区信息、物化视图等
GRANT SELECT ON DATABASE information_schema TO ROLE starrocks_admin;

-- 3. 授予 starrocks_audit_db__ 数据库的 SELECT 权限
-- 用途: 查询审计日志,分析查询历史、用户活跃度、性能统计等
GRANT SELECT ON DATABASE starrocks_audit_db__ TO ROLE starrocks_admin;

-- 4. 授予 SYSTEM 级别的 OPERATE 权限
-- 用途: 执行 SHOW PROC、SHOW PROCESSLIST 等集群监控命令
-- 权限范围: SHOW PROC '/backends', '/frontends', '/compactions' 等
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;

-- 5. 授予 _statistics_ 数据库的 SELECT 权限(可选)
-- 用途: 查询统计信息数据库(如果存在)
-- 注意: 某些版本的StarRocks可能不包含此数据库,可注释此行
-- GRANT SELECT ON DATABASE _statistics_ TO ROLE starrocks_admin;

-- ================================================================================
-- 完成提示
-- ================================================================================
-- 
-- ✅ 方案A配置完成!
-- 
-- 下一步:
--   1. 创建监控用户(见下方示例)
--   2. 执行权限验证SQL(见文末)
--   3. 在 starrocks-admin 项目中配置此用户的连接信息
--
-- ================================================================================


-- ================================================================================
-- 方案B: 兼容性权限配置(备选方案)
-- ================================================================================
--
-- 如果方案A的语法不被支持,请注释掉上面的方案A,启用下面的方案B
--

/*
-- 1. 创建角色
CREATE ROLE IF NOT EXISTS starrocks_admin;

-- 2. 授予全局 USAGE 权限(允许连接和基础SHOW命令)
GRANT USAGE ON *.* TO ROLE starrocks_admin;

-- 3. 授予 information_schema 的 SELECT 权限
GRANT SELECT ON information_schema.* TO ROLE starrocks_admin;

-- 4. 授予 starrocks_audit_db__ 的 SELECT 权限
GRANT SELECT ON starrocks_audit_db__.* TO ROLE starrocks_admin;

-- 5. 授予全局 OPERATE 权限(允许SHOW PROC等监控命令)
GRANT OPERATE ON *.* TO ROLE starrocks_admin;
*/

-- ================================================================================
-- 完成提示 (方案B)
-- ================================================================================
-- 
-- ✅ 方案B配置完成!
-- 
-- 注意: 方案B使用 *.* 语法,权限范围比方案A稍大,但兼容性更好
--
-- ================================================================================


-- ================================================================================
-- 方案C: 使用内置角色(快速部署,不推荐生产环境)
-- ================================================================================
--
-- 直接使用 StarRocks 内置的 db_admin 角色
-- 
-- ⚠️  警告: db_admin 包含所有数据库的管理权限(包括DDL操作)
-- ⚠️  不符合最小权限原则,仅适用于测试环境
--

/*
-- 如果选择方案C,只需在创建用户时直接授予 db_admin 角色:
-- CREATE USER 'starrocks_monitor'@'%' IDENTIFIED BY 'your_strong_password';
-- GRANT db_admin TO USER 'starrocks_monitor'@'%';
-- SET DEFAULT ROLE db_admin TO 'starrocks_monitor'@'%';
*/

-- ================================================================================
-- 不推荐原因 (方案C)
-- ================================================================================
-- 
-- db_admin 包含以下权限,远超监控需求:
--   - CREATE/DROP DATABASE/TABLE
--   - ALTER TABLE/DATABASE
--   - INSERT/UPDATE/DELETE 数据
--   - GRANT/REVOKE 权限管理
-- 
-- 生产环境请使用方案A或方案B
--
-- ================================================================================


-- ================================================================================
-- 创建监控用户并授予角色(示例)
-- ================================================================================
--
-- 请根据实际需求修改用户名、密码和IP限制
--

-- 示例1: 创建可从任意IP访问的监控用户
-- CREATE USER 'starrocks_monitor'@'%' 
--   IDENTIFIED BY 'Change_This_Strong_Password_123!';
-- GRANT starrocks_admin TO USER 'starrocks_monitor'@'%';
-- SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';

-- 示例2: 创建仅允许从特定IP访问的监控用户(更安全)
-- CREATE USER 'starrocks_monitor'@'192.168.1.100' 
--   IDENTIFIED BY 'Change_This_Strong_Password_123!';
-- GRANT starrocks_admin TO USER 'starrocks_monitor'@'192.168.1.100';
-- SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'192.168.1.100';

-- 示例3: 创建仅允许从特定IP段访问的监控用户
-- CREATE USER 'starrocks_monitor'@'192.168.1.%' 
--   IDENTIFIED BY 'Change_This_Strong_Password_123!';
-- GRANT starrocks_admin TO USER 'starrocks_monitor'@'192.168.1.%';
-- SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'192.168.1.%';

-- ================================================================================
-- 安全建议
-- ================================================================================
--
-- 1. 密码安全
--    - 使用强密码(至少16字符,包含大小写字母、数字、特殊字符)
--    - 不要使用默认密码或简单密码
--    - 定期更换密码(建议90天)
--
-- 2. 网络访问控制
--    - 优先使用IP限制(@'specific_ip')而不是@'%'
--    - 配置防火墙规则,仅允许监控服务器IP访问
--    - 考虑使用SSL/TLS加密连接
--
-- 3. 权限审计
--    - 定期检查角色权限: SHOW GRANTS FOR ROLE starrocks_admin;
--    - 定期检查用户权限: SHOW GRANTS FOR 'starrocks_monitor'@'%';
--    - 监控异常登录和查询行为
--
-- 4. 账号管理
--    - 不要使用root账号配置在监控平台
--    - 为每个监控平台实例创建独立账号
--    - 账号仅用于监控,不用于其他目的
--
-- 5. 默认角色设置
--    - 始终设置 DEFAULT ROLE,避免需要手动激活角色
--    - 监控账号不应拥有其他高权限角色
--
-- ================================================================================


-- ================================================================================
-- 权限验证SQL
-- ================================================================================
--
-- 使用监控用户登录后,执行以下SQL验证权限是否正确配置
--

-- 1. 验证角色权限
-- SHOW GRANTS FOR ROLE starrocks_admin;
-- 
-- 预期输出应包含:
--   - GRANT SELECT ON DATABASE information_schema TO ROLE 'starrocks_admin'
--   - GRANT SELECT ON DATABASE starrocks_audit_db__ TO ROLE 'starrocks_admin'
--   - GRANT OPERATE ON SYSTEM TO ROLE 'starrocks_admin'

-- 2. 验证用户拥有的角色
-- SHOW GRANTS FOR CURRENT_USER();
--
-- 预期输出应包含:
--   - GRANT 'starrocks_admin' TO USER 'starrocks_monitor'@'...'

-- 3. 验证当前激活的角色
-- SELECT CURRENT_ROLE();
--
-- 预期输出: starrocks_admin

-- 4. 测试查询 information_schema (应成功)
-- SELECT COUNT(*) FROM information_schema.tables;

-- 5. 测试查询审计日志 (应成功)
-- SELECT COUNT(*) FROM starrocks_audit_db__.starrocks_audit_tbl__ LIMIT 10;

-- 6. 测试 SHOW PROC 命令 (应成功)
-- SHOW PROC '/backends';

-- 7. 测试 SHOW PROCESSLIST (应成功)
-- SHOW PROCESSLIST;

-- 8. 测试查询业务数据库 (应失败,确认没有多余权限)
-- 假设有一个业务数据库叫 business_db
-- SELECT * FROM business_db.some_table LIMIT 1;
-- 预期: ERROR 1045 (HY000): Access denied

-- ================================================================================
-- 权限说明
-- ================================================================================
--
-- 此角色授予的权限及其用途:
--
-- 1. SELECT ON information_schema
--    - 查询表元数据: information_schema.tables
--    - 查询数据库列表: information_schema.schemata
--    - 查询分区信息: information_schema.partitions_meta
--    - 查询物化视图: information_schema.materialized_views
--    - 查询导入任务: information_schema.loads
--
-- 2. SELECT ON starrocks_audit_db__
--    - 查询审计日志: starrocks_audit_db__.starrocks_audit_tbl__
--    - 分析查询历史、计算QPS、延迟百分位等
--    - 统计活跃用户、热门表等
--
-- 3. OPERATE ON SYSTEM
--    - SHOW PROC '/backends' - 查看BE节点信息
--    - SHOW PROC '/frontends' - 查看FE节点信息
--    - SHOW PROC '/compactions' - 查看Compaction状态
--    - SHOW PROCESSLIST - 查看当前连接和查询
--    - SHOW FULL PROCESSLIST - 查看完整查询信息
--
-- 此角色不包含的权限(确保安全):
--    ✗ 查询用户业务数据库的数据
--    ✗ CREATE/DROP/ALTER 等DDL操作
--    ✗ INSERT/UPDATE/DELETE 等DML操作
--    ✗ GRANT/REVOKE 权限管理
--    ✗ ALTER SYSTEM 集群管理操作
--
-- ================================================================================


-- ================================================================================
-- 常见问题排查
-- ================================================================================
--
-- Q1: 执行脚本时提示 "Access denied"
-- A1: 确认当前用户拥有 user_admin 角色或 GRANT 权限
--     解决: 使用 root 用户或具有 user_admin 权限的用户执行
--
-- Q2: 监控用户连接时提示 "Access denied for user"
-- A2: 检查用户名、密码、IP限制是否正确
--     解决: SHOW GRANTS FOR 'username'@'host'; 确认用户是否创建成功
--
-- Q3: 查询 information_schema 时提示权限不足
-- A3: 角色可能没有激活
--     解决: SET ROLE starrocks_admin; 或设置为默认角色
--
-- Q4: SHOW PROC 命令提示 "Access denied"
-- A4: OPERATE 权限可能授予失败或语法不支持
--     解决: 尝试使用方案B的 GRANT OPERATE ON *.* 语法
--
-- Q5: 如何撤销权限?
-- A5: REVOKE SELECT ON DATABASE information_schema FROM ROLE starrocks_admin;
--     REVOKE OPERATE ON SYSTEM FROM ROLE starrocks_admin;
--
-- Q6: 如何删除角色?
-- A6: -- 先撤销所有用户的角色
--     REVOKE starrocks_admin FROM USER 'username'@'host';
--     -- 再删除角色
--     DROP ROLE starrocks_admin;
--
-- Q7: 查询审计日志时提示表不存在
-- A7: 审计日志功能可能未启用
--     解决: 在 fe.conf 中配置 enable_audit_log=true 并重启FE
--
-- Q8: 不同版本的StarRocks语法不兼容怎么办?
-- A8: 参考官方文档,使用对应版本的语法
--     3.0+: GRANT SELECT ON DATABASE db_name
--     2.x:  GRANT SELECT ON db_name.*
--
-- ================================================================================


-- ================================================================================
-- 版本兼容性说明
-- ================================================================================
--
-- StarRocks 3.0+:
--   - 支持 GRANT SELECT ON DATABASE 语法
--   - 支持 GRANT OPERATE ON SYSTEM 语法
--   - 推荐使用方案A
--
-- StarRocks 2.5:
--   - 使用 GRANT SELECT ON db_name.* 语法
--   - 使用 GRANT SELECT_PRIV, SHOW_PRIV 等旧语法
--   - 推荐使用方案B并调整语法
--
-- 如需支持旧版本,请参考官方文档调整语法:
-- https://docs.starrocks.io/zh/docs/administration/user_privs/
--
-- ================================================================================


-- ================================================================================
-- 项目配置示例
-- ================================================================================
--
-- 在 starrocks-admin 项目中配置监控用户:
--
-- 1. 修改配置文件 conf/config.toml (如果使用配置文件)
-- 2. 或在Web界面添加集群时填写以下信息:
--
--    集群名称: 生产集群
--    FE地址: your-fe-host
--    HTTP端口: 8030
--    查询端口: 9030
--    用户名: starrocks_monitor
--    密码: Change_This_Strong_Password_123!
--    Catalog: default_catalog
--    启用SSL: false (根据实际情况)
--
-- 3. 测试连接,确认能正常查询集群信息
--
-- ================================================================================


-- ================================================================================
-- 维护和升级
-- ================================================================================
--
-- 定期维护:
--   - 每季度审查权限配置,确认是否符合最小权限原则
--   - 每月检查监控用户的查询日志,识别异常行为
--   - 每90天更换一次密码
--
-- 升级注意事项:
--   - StarRocks升级后,检查权限语法是否有变化
--   - 测试所有监控功能是否正常工作
--   - 查看官方Release Notes中的权限系统变更
--
-- 备份权限配置:
--   -- 导出角色权限
--   SHOW GRANTS FOR ROLE starrocks_admin;
--   
--   -- 导出用户权限
--   SHOW GRANTS FOR 'starrocks_monitor'@'%';
--
-- ================================================================================


-- ================================================================================
-- 脚本结束
-- ================================================================================
-- 
-- 如有问题,请参考:
--   - StarRocks官方文档: https://docs.starrocks.io/
--   - 项目GitHub: https://github.com/your-org/starrocks-admin
--   - 提交Issue获取帮助
--
-- ================================================================================

