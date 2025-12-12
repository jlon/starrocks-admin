

```sql
CREATE USER 'starrocks_monitor'@'%' 
  IDENTIFIED BY 'Your_Strong_Password_Here';
GRANT starrocks_admin TO USER 'starrocks_monitor'@'%';
SET DEFAULT ROLE starrocks_admin TO 'starrocks_monitor'@'%';
```

**选项2: 使用现有用户**
```sql
-- 直接授予角色即可，不影响现有权限
GRANT starrocks_admin TO USER ''@'%';
SET GLOBAL activate_all_roles_on_login = TRUE;
```
```sql
CREATE ROLE starrocks_admin;
-- 授予查询系统表的权限
GRANT SELECT ON ALL TABLES IN DATABASE information_schema TO ROLE starrocks_admin;
-- 授予查询审计日志的权限
GRANT SELECT ON ALL TABLES IN DATABASE starrocks_audit_db__ TO ROLE starrocks_admin;
-- 授予系统操作权限(SHOW PROC等命令)
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;
```
```sql
CREATE ROLE starrocks_admin;
GRANT SELECT ON ALL TABLES IN DATABASE information_schema TO ROLE starrocks_admin;
GRANT SELECT ON ALL TABLES IN DATABASE starrocks_audit_db__ TO ROLE starrocks_admin;
GRANT OPERATE ON SYSTEM TO ROLE starrocks_admin;
SET GLOBAL activate_all_roles_on_login = TRUE;
```