# StarRocks Admin 权限配置指南

本文档说明如何为 StarRocks Admin 监控平台配置最小化的只读权限。

## 📋 目录

- [快速开始](#快速开始)
- [权限方案对比](#权限方案对比)
- [详细步骤](#详细步骤)
- [权限验证](#权限验证)
- [安全最佳实践](#安全最佳实践)
- [常见问题](#常见问题)
- [原理说明](#原理说明)

## 🚀 快速开始

### 1. 选择合适的方案

本项目提供三种权限配置方案：

| 方案 | 安全性 | 易用性 | 适用场景 |
|-----|--------|--------|---------|
| **方案A - 精确权限** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | 生产环境(推荐) |
| **方案B - 兼容权限** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | 版本兼容性需求 |
| **方案C - 内置角色** | ⭐⭐ | ⭐⭐⭐⭐⭐ | 测试环境 |

### 2. 执行配置脚本

```bash
# 使用root或具有user_admin权限的用户连接StarRocks
mysql -h <fe_host> -P 9030 -u root -p

# 执行权限配置脚本
source setup_starrocks_admin_role.sql
```

### 3. 创建监控用户

```sql
-- 创建用户(修改密码和IP限制)
CREATE USER 'starrocks_monitor'@'%' 
  IDENTIFIED BY 'Your_Strong_Password_Here';

-- 授予角色
GRANT starrocks_admin TO USER 'starrocks_monitor'@'%';

-- 设置默认角色
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';
```

### 4. 在项目中配置

在 StarRocks Admin Web界面中添加集群，使用刚创建的监控用户：

```
用户名: starrocks_monitor
密码: Your_Strong_Password_Here
```

## 📊 权限方案对比

### 方案A: 精确权限(推荐)

```sql
CREATE ROLE starrocks_admin;
GRANT SELECT ON DATABASE information_schema TO ROLE starrocks_admin;
GRANT SELECT ON DATABASE starrocks_audit_db__ TO ROLE starrocks_admin;
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;
```

**优点:**
- ✅ 最小权限原则,安全性最高
- ✅ 只能访问元数据和审计日志
- ✅ 无法访问用户业务数据
- ✅ 无法执行DDL/DML操作

**缺点:**
- ❌ 需要StarRocks 3.0+版本支持
- ❌ 语法较新,部分旧版本不兼容

**适用场景:**
- 生产环境
- 安全要求高的场景
- StarRocks 3.0+版本

### 方案B: 兼容权限

```sql
CREATE ROLE starrocks_admin;
GRANT USAGE ON *.* TO ROLE starrocks_admin;
GRANT SELECT ON information_schema.* TO ROLE starrocks_admin;
GRANT SELECT ON starrocks_audit_db__.* TO ROLE starrocks_admin;
GRANT OPERATE ON *.* TO ROLE starrocks_admin;
```

**优点:**
- ✅ 兼容性好,支持多版本StarRocks
- ✅ 权限范围相对较小
- ✅ 语法简单易懂

**缺点:**
- ❌ 使用 `*.*` 语法,权限范围稍大于方案A
- ❌ OPERATE权限是全局的

**适用场景:**
- StarRocks 2.x 版本
- 方案A语法不支持时的备选方案
- 需要兼容多个版本时

### 方案C: 内置角色

```sql
-- 直接授予db_admin角色
GRANT db_admin TO USER 'starrocks_monitor'@'%';
```

**优点:**
- ✅ 配置简单,一行代码完成
- ✅ 包含所有监控所需权限

**缺点:**
- ❌ 包含DDL权限(CREATE/DROP/ALTER)
- ❌ 包含DML权限(INSERT/UPDATE/DELETE)  
- ❌ 权限过大,不符合最小权限原则
- ❌ 存在误操作风险

**适用场景:**
- 仅限测试环境
- 快速功能验证
- 非生产环境

**⚠️ 警告:** 生产环境禁止使用此方案！

## 📝 详细步骤

### 步骤1: 准备工作

确认您有以下信息：

- [ ] StarRocks FE节点地址
- [ ] 具有管理权限的账号(root或user_admin角色)
- [ ] StarRocks版本号(决定使用哪个方案)

### 步骤2: 连接到StarRocks

```bash
# 方式1: 使用MySQL客户端
mysql -h <fe_host> -P 9030 -u root -p

# 方式2: 使用StarRocks客户端
# 下载地址: https://www.starrocks.io/download
```

### 步骤3: 执行权限配置

```sql
-- 查看当前用户权限(确认有足够权限)
SHOW GRANTS FOR CURRENT_USER();

-- 执行脚本创建角色
source /path/to/setup_starrocks_admin_role.sql

-- 或者直接复制粘贴SQL执行
```

### 步骤4: 创建监控用户

#### 选项A: 允许从任意IP访问

```sql
CREATE USER 'starrocks_monitor'@'%' 
  IDENTIFIED BY 'Your_Strong_Password_Here';
GRANT starrocks_admin TO USER 'starrocks_monitor'@'%';
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';
```

#### 选项B: 限制特定IP访问(更安全)

```sql
-- 单个IP
CREATE USER 'starrocks_monitor'@'192.168.1.100' 
  IDENTIFIED BY 'Your_Strong_Password_Here';
GRANT starrocks_admin TO USER 'starrocks_monitor'@'192.168.1.100';
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'192.168.1.100';

-- IP段
CREATE USER 'starrocks_monitor'@'192.168.1.%' 
  IDENTIFIED BY 'Your_Strong_Password_Here';
GRANT starrocks_admin TO USER 'starrocks_monitor'@'192.168.1.%';
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'192.168.1.%';
```

### 步骤5: 验证配置

```sql
-- 1. 检查角色权限
SHOW GRANTS FOR ROLE starrocks_admin;

-- 2. 检查用户权限
SHOW GRANTS FOR 'starrocks_monitor'@'%';

-- 3. 使用监控用户登录测试
-- 退出当前连接,使用监控用户重新连接
exit;
mysql -h <fe_host> -P 9030 -u starrocks_monitor -p

-- 4. 测试查询(应该成功)
SELECT COUNT(*) FROM information_schema.tables;
SHOW PROC '/backends';
```

## ✅ 权限验证

使用监控用户登录后,执行以下测试:

### 应该成功的操作

```sql
-- ✅ 查询元数据
SELECT * FROM information_schema.tables LIMIT 10;
SELECT * FROM information_schema.schemata;

-- ✅ 查询审计日志  
SELECT * FROM starrocks_audit_db__.starrocks_audit_tbl__ LIMIT 10;

-- ✅ 查看集群信息
SHOW PROC '/backends';
SHOW PROC '/frontends';
SHOW PROCESSLIST;
SHOW DATABASES;

-- ✅ 查询系统信息
SELECT VERSION();
SELECT CURRENT_ROLE();
```

### 应该失败的操作

```sql
-- ❌ 查询业务数据(假设有业务数据库test_db)
SELECT * FROM test_db.some_table;
-- 预期错误: ERROR 1045 (HY000): Access denied

-- ❌ 创建数据库
CREATE DATABASE test_db;
-- 预期错误: ERROR 1045 (HY000): Access denied

-- ❌ 删除表
DROP TABLE test_db.some_table;
-- 预期错误: ERROR 1045 (HY000): Access denied

-- ❌ 插入数据
INSERT INTO test_db.some_table VALUES (1);
-- 预期错误: ERROR 1045 (HY000): Access denied
```

如果以上测试结果符合预期,说明权限配置正确！

## 🔒 安全最佳实践

### 1. 密码安全

```sql
-- ✅ 好的密码示例
'MyS3cur3P@ssw0rd!2024#SR'

-- ❌ 不好的密码示例  
'123456'
'password'
'starrocks'
```

**密码要求:**
- 至少16个字符
- 包含大小写字母、数字、特殊字符
- 不使用字典词汇
- 不使用个人信息
- 定期更换(建议90天)

### 2. 网络访问控制

```sql
-- ✅ 推荐: 限制IP访问
CREATE USER 'monitor'@'192.168.1.100' IDENTIFIED BY '...';

-- ⚠️ 谨慎: 允许IP段访问
CREATE USER 'monitor'@'192.168.1.%' IDENTIFIED BY '...';

-- ❌ 不推荐: 允许任意IP访问(仅测试环境)
CREATE USER 'monitor'@'%' IDENTIFIED BY '...';
```

**额外防护:**
- 配置防火墙规则
- 使用VPN或专线访问
- 启用SSL/TLS加密连接

### 3. 权限审计

```sql
-- 定期检查角色权限
SHOW GRANTS FOR ROLE starrocks_admin;

-- 定期检查用户权限
SHOW GRANTS FOR 'starrocks_monitor'@'%';

-- 查看最近的登录和查询
SELECT * FROM starrocks_audit_db__.starrocks_audit_tbl__
WHERE user = 'starrocks_monitor'
ORDER BY timestamp DESC
LIMIT 100;
```

**审计频率:**
- 每月审查一次权限配置
- 每周检查异常查询
- 实时监控失败的登录尝试

### 4. 账号管理

- ✅ 为每个监控平台实例创建独立账号
- ✅ 账号仅用于监控,不共享给其他用途
- ✅ 离职员工及时删除相关账号
- ❌ 不要使用root账号配置监控平台
- ❌ 不要在多个系统共用同一账号

### 5. 默认角色设置

```sql
-- ✅ 正确: 设置默认角色
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';

-- ❌ 错误: 不设置默认角色
-- 需要每次手动激活: SET ROLE starrocks_admin;
```

## ❓ 常见问题

### Q1: 执行脚本时提示 "Access denied"

**原因:** 当前用户没有创建角色和授权的权限

**解决方案:**
```sql
-- 方案1: 使用root用户
mysql -h <fe_host> -P 9030 -u root -p

-- 方案2: 授予当前用户user_admin角色
-- (需要root用户执行)
GRANT user_admin TO USER 'your_user'@'%';
```

### Q2: 监控用户无法查询 information_schema

**原因:** 角色未激活或默认角色未设置

**解决方案:**
```sql
-- 检查当前角色
SELECT CURRENT_ROLE();

-- 如果为空,手动激活
SET ROLE starrocks_admin;

-- 或设置为默认角色(root用户执行)
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';
```

### Q3: SHOW PROC 命令提示权限不足

**原因:** OPERATE权限未正确授予

**解决方案:**
```sql
-- 检查角色权限
SHOW GRANTS FOR ROLE starrocks_admin;

-- 如果缺少OPERATE权限,补充授予
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;

-- 或使用方案B语法
GRANT OPERATE ON *.* TO ROLE starrocks_admin;
```

### Q4: 查询审计日志时提示表不存在

**原因:** 审计日志功能未启用

**解决方案:**
```bash
# 1. 编辑FE配置文件
vi fe/conf/fe.conf

# 2. 添加或修改配置
enable_audit_log = true

# 3. 重启FE节点
fe/bin/stop_fe.sh
fe/bin/start_fe.sh --daemon

# 4. 验证审计日志表是否创建
mysql -h <fe_host> -P 9030 -u root -p
SHOW TABLES FROM starrocks_audit_db__;
```

### Q5: 项目无法连接StarRocks

**检查清单:**

1. 网络连通性
```bash
# 测试FE HTTP端口(默认8030)
telnet <fe_host> 8030

# 测试FE Query端口(默认9030)
telnet <fe_host> 9030
```

2. 用户名密码正确性
```sql
-- 使用mysql客户端测试
mysql -h <fe_host> -P 9030 -u starrocks_monitor -p
```

3. IP访问限制
```sql
-- 检查用户的host配置
SELECT user, host FROM mysql.user WHERE user = 'starrocks_monitor';

-- 如果host='192.168.1.100',但项目从其他IP访问会失败
-- 解决: 修改为允许的IP或使用'%'
```

4. 防火墙规则
```bash
# 检查防火墙是否允许访问
sudo firewall-cmd --list-all  # CentOS/RHEL
sudo ufw status              # Ubuntu
```

### Q6: 如何修改监控用户密码?

```sql
-- 方案1: 使用ALTER USER(推荐)
ALTER USER 'starrocks_monitor'@'%' IDENTIFIED BY 'New_Strong_Password';

-- 方案2: 使用SET PASSWORD
SET PASSWORD FOR 'starrocks_monitor'@'%' = PASSWORD('New_Strong_Password');

-- 修改后,更新项目中的配置
```

### Q7: 如何删除角色和用户?

```sql
-- 1. 先撤销用户的角色
REVOKE starrocks_admin FROM USER 'starrocks_monitor'@'%';

-- 2. 删除用户
DROP USER 'starrocks_monitor'@'%';

-- 3. 删除角色
DROP ROLE starrocks_admin;
```

### Q8: 不同版本StarRocks语法不兼容

**StarRocks 3.0+ (新版本)**
```sql
GRANT SELECT ON DATABASE information_schema TO ROLE starrocks_admin;
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;
```

**StarRocks 2.x (旧版本)**
```sql
GRANT SELECT_PRIV ON information_schema.* TO ROLE starrocks_admin;
GRANT OPERATE_PRIV ON *.* TO ROLE starrocks_admin;
```

**解决方案:** 查看官方文档对应版本的语法
- https://docs.starrocks.io/zh/docs/administration/user_privs/

## 📚 原理说明

### 项目需要哪些权限?

StarRocks Admin 是一个**只读监控平台**,通过查询元数据和审计日志来提供集群监控能力。

#### 1. 元数据查询 (information_schema)

```sql
-- 查询表信息(大小、行数)
SELECT * FROM information_schema.tables;

-- 查询数据库列表
SELECT * FROM information_schema.schemata;

-- 查询分区信息(Compaction Score)
SELECT * FROM information_schema.partitions_meta;

-- 查询物化视图
SELECT * FROM information_schema.materialized_views;

-- 查询导入任务
SELECT * FROM information_schema.loads;
```

**用途:**
- 显示数据库和表列表
- 统计数据大小和增长趋势
- 监控Compaction状态
- 管理物化视图

#### 2. 审计日志查询 (starrocks_audit_db__)

```sql
-- 查询历史查询记录
SELECT * FROM starrocks_audit_db__.starrocks_audit_tbl__
WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 1 DAY);

-- 计算QPS
SELECT DATE_FORMAT(timestamp, '%Y-%m-%d %H:%i:00') as time_bucket,
       COUNT(*) / 60 as qps
FROM starrocks_audit_db__.starrocks_audit_tbl__
GROUP BY time_bucket;

-- 计算查询延迟百分位
SELECT percentile_approx(queryTime, 0.99) as p99_latency
FROM starrocks_audit_db__.starrocks_audit_tbl__
WHERE timestamp >= DATE_SUB(NOW(), INTERVAL 1 HOUR);
```

**用途:**
- 计算QPS、RPS指标
- 分析查询延迟(P50/P95/P99)
- 统计活跃用户数
- 识别慢查询和热门表

#### 3. 集群监控 (SHOW PROC)

```sql
-- 查看BE节点状态
SHOW PROC '/backends';

-- 查看FE节点状态
SHOW PROC '/frontends';

-- 查看Compaction任务
SHOW PROC '/compactions';

-- 查看当前连接
SHOW PROCESSLIST;
```

**用途:**
- 监控节点健康状态
- 统计节点资源使用
- 查看运行中的查询
- 监控Compaction进度

### 为什么不直接使用db_admin?

`db_admin` 是StarRocks的内置角色,拥有以下权限:

| 权限类型 | db_admin | starrocks_admin | 说明 |
|---------|----------|-----------------|------|
| SELECT | ✅ 所有库 | ✅ 仅元数据库 | 监控只需元数据 |
| CREATE | ✅ | ❌ | 监控不需要创建权限 |
| DROP | ✅ | ❌ | 监控不需要删除权限 |
| ALTER | ✅ | ❌ | 监控不需要修改权限 |
| INSERT | ✅ | ❌ | 监控不需要写入权限 |
| DELETE | ✅ | ❌ | 监控不需要删除数据 |

**结论:** `db_admin` 权限过大,违反最小权限原则,生产环境应使用自定义的 `starrocks_admin` 角色。

### HTTP API 权限说明

项目还会访问以下HTTP接口:

```bash
# Prometheus指标
GET http://<fe_host>:8030/metrics

# 集群状态(SHOW PROC的HTTP接口)
GET http://<fe_host>:8030/api/show_proc?path=/backends

# 运行时信息
GET http://<fe_host>:8030/api/bootstrap
```

这些HTTP接口使用**HTTP Basic Auth**验证,与数据库权限独立:
- 只要用户名密码正确,就能访问
- 不受数据库权限系统控制
- 因此无需额外的数据库权限

## 📖 相关文档

- [StarRocks 权限管理官方文档](https://docs.starrocks.io/zh/docs/3.5/administration/user_privs/authorization/)
- [StarRocks Admin 项目文档](../README.md)
- [StarRocks 审计日志配置](https://docs.starrocks.io/zh/docs/administration/management/audit_log/)

## 🤝 获取帮助

如果遇到问题:

1. 查看本文档的[常见问题](#常见问题)章节
2. 查看脚本中的详细注释
3. 提交 GitHub Issue
4. 加入社区讨论群

---

**最后更新:** 2025-11-03  
**脚本版本:** 1.0.0  
**适用版本:** StarRocks 3.0+

