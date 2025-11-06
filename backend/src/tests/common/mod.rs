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

    // Create test permissions (using INSERT OR IGNORE to handle duplicates)
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description)
        VALUES 
        ('menu:dashboard', 'Dashboard', 'menu', 'dashboard', NULL, 'Dashboard menu'),
        ('menu:overview', 'Overview', 'menu', 'overview', NULL, 'Overview menu'),
        ('menu:users', 'Users', 'menu', 'users', NULL, 'Users menu'),
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
        ('api:users:update', 'Update User', 'api', 'users', 'update', 'Update user API')
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

/// Create a test user
pub async fn create_test_user(pool: &SqlitePool, username: &str) -> i64 {
    sqlx::query("INSERT INTO users (username, password_hash, email) VALUES (?, ?, ?)")
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

    tx.commit().await.expect("Failed to commit permission grants");
}
