// Common test utilities and helpers

use crate::services::casbin_service::CasbinService;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::sync::Arc;
use std::time::Duration;

/// Create an in-memory SQLite database for testing
pub async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(3))
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");

    // Run migrations
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Create a test Casbin service
pub async fn create_test_casbin_service() -> Arc<CasbinService> {
    Arc::new(
        CasbinService::new()
            .await
            .expect("Failed to create Casbin service"),
    )
}

/// Setup test data: roles, permissions, and relationships
pub struct TestData {
    pub admin_role_id: i64,
    pub permission_ids: Vec<i64>,
}

/// Multi-tenant test data structure
pub struct MultiTenantTestData {
    pub super_admin_user_id: i64,
    pub org1_id: i64,
    pub org1_admin_user_id: i64,
    pub org1_regular_user_id: i64,
    pub org2_id: i64,
    pub org2_admin_user_id: i64,
    pub org2_regular_user_id: i64,
    pub super_admin_role_id: i64,
    pub regular_role_id: i64,
}

pub async fn setup_test_data(pool: &SqlitePool) -> TestData {
    // Clear existing test data first
    sqlx::query("DELETE FROM user_roles")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM role_permissions")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM roles").execute(pool).await.ok();
    sqlx::query("DELETE FROM permissions")
        .execute(pool)
        .await
        .ok();

    sqlx::query(
        "INSERT INTO organizations (code, name, description, is_system) VALUES ('default_org', 'Default Organization', 'System default organization (test seed)', 1)",
    )
    .execute(pool)
    .await
    .ok();

    // Create test permissions (using INSERT OR IGNORE to handle duplicates)
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description)
        VALUES 
        ('menu:dashboard', 'Dashboard', 'menu', 'dashboard', NULL, 'Dashboard menu'),
        ('menu:overview', 'Overview', 'menu', 'overview', NULL, 'Overview menu'),
        ('menu:system', 'System Management', 'menu', 'system', NULL, 'System management parent menu'),
        ('menu:system:users', 'Users', 'menu', 'system:users', NULL, 'Users menu'),
        ('menu:system:roles', 'Roles', 'menu', 'system:roles', NULL, 'Roles menu'),
        ('api:clusters:create', 'Create Cluster', 'api', 'clusters', 'create', 'Create cluster API'),
        ('api:clusters:delete', 'Delete Cluster', 'api', 'clusters', 'delete', 'Delete cluster API'),
        ('api:clusters:update', 'Update Cluster', 'api', 'clusters', 'update', 'Update cluster API'),
        ('api:clusters:get', 'Get Cluster', 'api', 'clusters', 'get', 'Get cluster API'),
        ('api:clusters:list', 'List Clusters', 'api', 'clusters', 'list', 'List clusters API'),
        ('api:roles:list', 'List Roles', 'api', 'roles', 'list', 'List roles API'),
        ('api:roles:create', 'Create Role', 'api', 'roles', 'create', 'Create role API'),
        ('api:roles:get', 'Get Role', 'api', 'roles', 'get', 'Get role API'),
        ('api:roles:update', 'Update Role', 'api', 'roles', 'update', 'Update role API'),
        ('api:roles:delete', 'Delete Role', 'api', 'roles', 'delete', 'Delete role API'),
        ('api:users:update', 'Update User', 'api', 'users', 'update', 'Update user API'),
        ('api:organizations:list', 'List Organizations', 'api', 'organizations', 'list', 'List organizations API'),
        ('api:organizations:create', 'Create Organization', 'api', 'organizations', 'create', 'Create organization API'),
        ('api:organizations:get', 'Get Organization', 'api', 'organizations', 'get', 'Get organization API'),
        ('api:organizations:update', 'Update Organization', 'api', 'organizations', 'update', 'Update organization API'),
        ('api:organizations:delete', 'Delete Organization', 'api', 'organizations', 'delete', 'Delete organization API')
        "#
    )
    .execute(pool)
    .await
    .expect("Failed to insert test permissions");

    // Get permission IDs
    let permissions: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, code FROM permissions ORDER BY code")
            .fetch_all(pool)
            .await
            .expect("Failed to fetch permissions");

    // Create test roles (using INSERT OR IGNORE to handle duplicates)
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO roles (code, name, description, is_system)
        VALUES ('admin', 'Administrator', 'System administrator role', 1)
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert admin role");

    // Get role IDs
    let (admin_role_id,): (i64,) = sqlx::query_as("SELECT id FROM roles WHERE code = ?")
        .bind("admin")
        .fetch_one(pool)
        .await
        .expect("Failed to fetch admin role");

    // Assign permissions to admin role (all permissions)
    for (perm_id, _) in &permissions {
        sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)")
            .bind(admin_role_id)
            .bind(perm_id)
            .execute(pool)
            .await
            .expect("Failed to assign permissions to admin role");
    }

    let permission_ids: Vec<i64> = permissions.iter().map(|(id, _)| *id).collect();
    TestData { admin_role_id, permission_ids }
}

/// Setup comprehensive multi-tenant test data
pub async fn setup_multi_tenant_test_data(pool: &SqlitePool) -> MultiTenantTestData {
    // Clear all data
    sqlx::query("DELETE FROM user_organizations")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM user_roles")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM role_permissions")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM organizations")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM users").execute(pool).await.ok();
    sqlx::query("DELETE FROM roles").execute(pool).await.ok();
    sqlx::query("DELETE FROM permissions")
        .execute(pool)
        .await
        .ok();

    // Create permissions
    sqlx::query(
        r#"
        INSERT INTO permissions (code, name, type, resource, action, description)
        VALUES 
        ('api:organizations:list', 'List Organizations', 'api', 'organizations', 'list', 'List organizations API'),
        ('api:organizations:create', 'Create Organization', 'api', 'organizations', 'create', 'Create organization API'),
        ('api:organizations:get', 'Get Organization', 'api', 'organizations', 'get', 'Get organization API'),
        ('api:organizations:update', 'Update Organization', 'api', 'organizations', 'update', 'Update organization API'),
        ('api:organizations:delete', 'Delete Organization', 'api', 'organizations', 'delete', 'Delete organization API'),
        ('api:users:list', 'List Users', 'api', 'users', 'list', 'List users API'),
        ('api:users:create', 'Create User', 'api', 'users', 'create', 'Create user API'),
        ('api:users:get', 'Get User', 'api', 'users', 'get', 'Get user API'),
        ('api:users:update', 'Update User', 'api', 'users', 'update', 'Update user API'),
        ('api:users:delete', 'Delete User', 'api', 'users', 'delete', 'Delete user API'),
        ('api:roles:list', 'List Roles', 'api', 'roles', 'list', 'List roles API'),
        ('api:roles:create', 'Create Role', 'api', 'roles', 'create', 'Create role API'),
        ('api:roles:get', 'Get Role', 'api', 'roles', 'get', 'Get role API'),
        ('api:roles:update', 'Update Role', 'api', 'roles', 'update', 'Update role API'),
        ('api:roles:delete', 'Delete Role', 'api', 'roles', 'delete', 'Delete role API')
        "#
    )
    .execute(pool)
    .await
    .expect("Failed to insert permissions");

    let permissions: Vec<(i64, String)> = sqlx::query_as("SELECT id, code FROM permissions")
        .fetch_all(pool)
        .await
        .expect("Failed to fetch permissions");

    // Create super admin role (system-wide, no organization)
    let super_admin_role_id = create_role(
        pool,
        "super_admin",
        "Super Administrator",
        "System super administrator with full access",
        true,
    )
    .await;

    // Create org admin role (organization-scoped)
    let org_admin_role_id = create_role(
        pool,
        "org_admin",
        "Organization Administrator",
        "Organization administrator with org-scoped access",
        false,
    )
    .await;

    // Create regular user role (organization-scoped)
    let regular_role_id = create_role(
        pool,
        "regular_user",
        "Regular User",
        "Regular user with limited access",
        false,
    )
    .await;

    // Grant all permissions to super admin role
    let all_permission_ids: Vec<i64> = permissions.iter().map(|(id, _)| *id).collect();
    grant_permissions(pool, super_admin_role_id, &all_permission_ids).await;

    // Grant organization-scoped permissions to org admin role
    let org_permission_ids: Vec<i64> = permissions
        .iter()
        .filter(|(_, code)| {
            code.contains("organizations") || code.contains("users") || code.contains("roles")
        })
        .map(|(id, _)| *id)
        .collect();
    grant_permissions(pool, org_admin_role_id, &org_permission_ids).await;

    // Grant limited permissions to regular user role
    let user_permission_ids: Vec<i64> = permissions
        .iter()
        .filter(|(_, code)| code.contains("users:get") || code.contains("roles:get"))
        .map(|(id, _)| *id)
        .collect();
    grant_permissions(pool, regular_role_id, &user_permission_ids).await;

    // Create super admin user (no organization)
    let super_admin_user_id = create_test_user(pool, "super_admin").await;
    // Ensure super admin stays organization-neutral
    sqlx::query("UPDATE users SET organization_id = NULL WHERE id = ?")
        .bind(super_admin_user_id)
        .execute(pool)
        .await
        .expect("Failed to reset super admin organization");
    sqlx::query("DELETE FROM user_organizations WHERE user_id = ?")
        .bind(super_admin_user_id)
        .execute(pool)
        .await
        .ok();
    assign_role_to_user(pool, super_admin_user_id, super_admin_role_id).await;

    // Create organization 1
    let org1_id =
        create_test_organization(pool, "org1", "Organization 1", "First test organization").await;

    // Create organization 1 admin user
    let org1_admin_user_id = create_test_user_with_org(pool, "org1_admin", org1_id).await;
    assign_role_to_user(pool, org1_admin_user_id, org_admin_role_id).await;
    assign_user_to_organization(pool, org1_admin_user_id, org1_id).await;

    // Create organization 1 regular user
    let org1_regular_user_id = create_test_user_with_org(pool, "org1_regular", org1_id).await;
    assign_role_to_user(pool, org1_regular_user_id, regular_role_id).await;
    assign_user_to_organization(pool, org1_regular_user_id, org1_id).await;

    // Create organization 2
    let org2_id =
        create_test_organization(pool, "org2", "Organization 2", "Second test organization").await;

    // Create organization 2 admin user
    let org2_admin_user_id = create_test_user_with_org(pool, "org2_admin", org2_id).await;
    assign_role_to_user(pool, org2_admin_user_id, org_admin_role_id).await;
    assign_user_to_organization(pool, org2_admin_user_id, org2_id).await;

    // Create organization 2 regular user
    let org2_regular_user_id = create_test_user_with_org(pool, "org2_regular", org2_id).await;
    assign_role_to_user(pool, org2_regular_user_id, regular_role_id).await;
    assign_user_to_organization(pool, org2_regular_user_id, org2_id).await;

    // Update roles with organization IDs
    sqlx::query("UPDATE roles SET organization_id = ? WHERE code IN ('org_admin', 'regular_user')")
        .bind(org1_id)
        .execute(pool)
        .await
        .expect("Failed to update roles with organization_id");

    MultiTenantTestData {
        super_admin_user_id,
        org1_id,
        org1_admin_user_id,
        org1_regular_user_id,
        org2_id,
        org2_admin_user_id,
        org2_regular_user_id,
        super_admin_role_id,
        regular_role_id,
    }
}

/// Create a test organization
pub async fn create_test_organization(
    pool: &SqlitePool,
    code: &str,
    name: &str,
    description: &str,
) -> i64 {
    sqlx::query(
        "INSERT INTO organizations (code, name, description, is_system) VALUES (?, ?, ?, 0)",
    )
    .bind(code)
    .bind(name)
    .bind(description)
    .execute(pool)
    .await
    .expect("Failed to create test organization");

    let (id,): (i64,) = sqlx::query_as("SELECT id FROM organizations WHERE code = ?")
        .bind(code)
        .fetch_one(pool)
        .await
        .expect("Failed to fetch test organization");

    id
}

/// Create a test user with organization
pub async fn create_test_user_with_org(
    pool: &SqlitePool,
    username: &str,
    organization_id: i64,
) -> i64 {
    sqlx::query(
        "INSERT INTO users (username, password_hash, email, organization_id) VALUES (?, ?, ?, ?)",
    )
    .bind(username)
    .bind("$2b$12$hashed_password") // Dummy hash
    .bind(format!("{}@test.com", username))
    .bind(organization_id)
    .execute(pool)
    .await
    .expect("Failed to create test user");

    let user: (i64,) = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(username)
        .fetch_one(pool)
        .await
        .expect("Failed to fetch test user");

    user.0
}

/// Assign user to organization
pub async fn assign_user_to_organization(pool: &SqlitePool, user_id: i64, organization_id: i64) {
    sqlx::query(
        r#"
        INSERT INTO user_organizations (user_id, organization_id)
        VALUES (?, ?)
        ON CONFLICT(user_id) DO UPDATE SET organization_id = excluded.organization_id
        "#,
    )
    .bind(user_id)
    .bind(organization_id)
    .execute(pool)
    .await
    .expect("Failed to assign user to organization");

    sqlx::query("UPDATE users SET organization_id = ? WHERE id = ?")
        .bind(organization_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("Failed to update user organization");
}

/// Create a test user
pub async fn create_test_user(pool: &SqlitePool, username: &str) -> i64 {
    sqlx::query(
        "INSERT INTO users (username, password_hash, email, organization_id) VALUES (?, ?, ?, NULL)",
    )
        .bind(username)
        .bind("$2b$12$hashed_password") // Dummy hash
        .bind(format!("{}@test.com", username))
        .execute(pool)
        .await
        .expect("Failed to create test user");

    let user: (i64,) = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(username)
        .fetch_one(pool)
        .await
        .expect("Failed to fetch test user");

    user.0
}

/// Assign role to user
pub async fn assign_role_to_user(pool: &SqlitePool, user_id: i64, role_id: i64) {
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await
        .expect("Failed to assign role to user");
}

/// Create a custom role for tests
pub async fn create_role(
    pool: &SqlitePool,
    code: &str,
    name: &str,
    description: &str,
    is_system: bool,
) -> i64 {
    sqlx::query("INSERT INTO roles (code, name, description, is_system) VALUES (?, ?, ?, ?)")
        .bind(code)
        .bind(name)
        .bind(description)
        .bind(if is_system { 1 } else { 0 })
        .execute(pool)
        .await
        .expect("Failed to insert custom role");

    let (id,): (i64,) = sqlx::query_as("SELECT id FROM roles WHERE code = ?")
        .bind(code)
        .fetch_one(pool)
        .await
        .expect("Failed to fetch custom role id");

    id
}

/// Grant permissions to role (replaces existing assignments)
pub async fn grant_permissions(pool: &SqlitePool, role_id: i64, permission_ids: &[i64]) {
    let mut tx = pool.begin().await.expect("Failed to begin transaction");

    sqlx::query("DELETE FROM role_permissions WHERE role_id = ?")
        .bind(role_id)
        .execute(&mut *tx)
        .await
        .expect("Failed to clear role permissions");

    for permission_id in permission_ids {
        sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)")
            .bind(role_id)
            .bind(permission_id)
            .execute(&mut *tx)
            .await
            .expect("Failed to grant permission to role");
    }

    tx.commit()
        .await
        .expect("Failed to commit permission grants");
}
