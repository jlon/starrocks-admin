# StarRocks Admin 权限配置指南

## 创建管理员角色

```sql
CREATE ROLE starrocks_admin;

-- 授予查询系统表的权限
GRANT SELECT ON ALL TABLES IN DATABASE information_schema TO ROLE starrocks_admin;

-- 授予查询审计日志的权限
GRANT SELECT ON ALL TABLES IN DATABASE starrocks_audit_db__ TO ROLE starrocks_admin;

-- 授予系统操作权限(SHOW PROC等命令)
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;

-- 授予 SQL 黑名单管理权限（可选，用于管理大查询黑名单）
GRANT BLACKLIST ON SYSTEM TO ROLE starrocks_admin;
```

## 创建用户并授权

**选项1: 创建新用户**
```sql
CREATE USER 'starrocks_monitor'@'%' 
  IDENTIFIED BY 'Your_Strong_Password_Here';
GRANT starrocks_admin TO USER 'starrocks_monitor'@'%';
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';
```

**选项2: 使用现有用户**
```sql
-- 直接授予角色即可，不影响现有权限
GRANT starrocks_admin TO USER '<username>'@'%';
SET GLOBAL activate_all_roles_on_login = TRUE;
```

## 启用 SQL 黑名单功能

如需使用 SQL 黑名单功能，需要先启用：
```sql
ADMIN SET FRONTEND CONFIG ("enable_sql_blacklist" = "true");
```
用户需要有 BLACKLIST 权限才能管理黑名单：
```
GRANT BLACKLIST ON SYSTEM TO <user>;
```