use crate::services::casbin_service::CasbinService;
use crate::tests::common::{create_test_casbin_service, create_test_db, setup_test_data};

#[tokio::test]
async fn test_casbin_service_new() {
    let service = CasbinService::new().await;
    assert!(service.is_ok(), "CasbinService should initialize successfully");
}

#[tokio::test]
async fn test_casbin_service_enforce_without_policies() {
    let service = create_test_casbin_service().await;

    // Without any policies, enforce should return false
    let result = service.enforce(1, "clusters", "create").await;
    assert!(result.is_ok());
    assert!(!result.unwrap(), "Should deny without policies");
}

#[tokio::test]
async fn test_casbin_service_add_and_enforce_policy() {
    let service = create_test_casbin_service().await;

    // Add policy: role 1 can create clusters
    let result = service.add_policy(1, "clusters", "create").await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Policy should be added");

    // Add role assignment: user 100 has role 1
    let result = service.add_role_for_user(100, 1).await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Role assignment should be added");

    // Now enforce should return true
    let result = service.enforce(100, "clusters", "create").await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Should allow with matching policy");
}

#[tokio::test]
async fn test_casbin_service_remove_policy() {
    let service = create_test_casbin_service().await;

    // Add policy
    service.add_policy(1, "clusters", "delete").await.unwrap();
    service.add_role_for_user(200, 1).await.unwrap();

    // Verify it works
    assert!(service.enforce(200, "clusters", "delete").await.unwrap());

    // Remove policy
    let result = service.remove_policy(1, "clusters", "delete").await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Policy should be removed");

    // Verify it no longer works
    assert!(!service.enforce(200, "clusters", "delete").await.unwrap());
}

#[tokio::test]
async fn test_casbin_service_add_remove_role_for_user() {
    let service = create_test_casbin_service().await;

    // Add policy for role 2
    service.add_policy(2, "users", "update").await.unwrap();

    // Add role assignment
    let result = service.add_role_for_user(300, 2).await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Role assignment should be added");

    // Verify user can access
    assert!(service.enforce(300, "users", "update").await.unwrap());

    // Remove role assignment
    let result = service.remove_role_for_user(300, 2).await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Role assignment should be removed");

    // Verify user can no longer access
    assert!(!service.enforce(300, "users", "update").await.unwrap());
}

#[tokio::test]
async fn test_casbin_service_multiple_policies() {
    let service = create_test_casbin_service().await;

    // Add multiple policies for different roles
    service.add_policy(1, "clusters", "create").await.unwrap();
    service.add_policy(1, "clusters", "delete").await.unwrap();
    service.add_policy(2, "users", "update").await.unwrap();

    // Assign multiple roles to user
    service.add_role_for_user(400, 1).await.unwrap();
    service.add_role_for_user(400, 2).await.unwrap();

    // User should have access to all policies
    assert!(service.enforce(400, "clusters", "create").await.unwrap());
    assert!(service.enforce(400, "clusters", "delete").await.unwrap());
    assert!(service.enforce(400, "users", "update").await.unwrap());
}

#[tokio::test]
async fn test_casbin_service_reload_policies_from_db() {
    let pool = create_test_db().await;
    let service = create_test_casbin_service().await;

    // Setup test data with roles and permissions
    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;

    // Create a user and assign admin role
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;
    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;

    // Reload policies from database
    let result = service.reload_policies_from_db(&pool).await;
    assert!(result.is_ok(), "Should reload policies successfully");

    // User should have access to admin permissions
    // Based on reload_policies_from_db logic:
    // - "api:clusters:create" maps to resource="system:clusters", action="create"
    // - "menu:dashboard" maps to resource="system:dashboard", action="view"
    let has_cluster_permission = service.enforce(user_id, "system:clusters", "create").await;
    assert!(has_cluster_permission.is_ok(), "Permission check should succeed");
    assert!(has_cluster_permission.unwrap(), "Admin should have cluster:create permission");
}

#[tokio::test]
async fn test_casbin_service_reload_clears_old_policies() {
    let pool = create_test_db().await;
    let service = create_test_casbin_service().await;

    // Add a manual policy
    service.add_policy(999, "test", "action").await.unwrap();
    service.add_role_for_user(999, 999).await.unwrap();

    // Setup test data and reload
    setup_test_data(&pool).await;
    service.reload_policies_from_db(&pool).await.unwrap();

    // Old manual policy should be cleared
    assert!(!service.enforce(999, "test", "action").await.unwrap());
}

#[tokio::test]
async fn test_casbin_service_enforce_different_actions() {
    let service = create_test_casbin_service().await;

    service.add_policy(1, "clusters", "create").await.unwrap();
    service.add_policy(1, "clusters", "read").await.unwrap();
    service.add_role_for_user(500, 1).await.unwrap();

    // User should have create access
    assert!(service.enforce(500, "clusters", "create").await.unwrap());

    // User should have read access
    assert!(service.enforce(500, "clusters", "read").await.unwrap());

    // User should NOT have delete access
    assert!(!service.enforce(500, "clusters", "delete").await.unwrap());
}

#[tokio::test]
async fn test_casbin_service_double_add_policy() {
    let service = create_test_casbin_service().await;

    // Add same policy twice
    let result1 = service.add_policy(1, "test", "action").await.unwrap();
    let _result2 = service.add_policy(1, "test", "action").await.unwrap();

    // First should return true, second might return false (already exists)
    assert!(result1);
    // Second might be false if Casbin prevents duplicates
}

#[tokio::test]
async fn test_casbin_service_double_add_role() {
    let service = create_test_casbin_service().await;

    // Add same role assignment twice
    let result1 = service.add_role_for_user(600, 1).await.unwrap();
    let _result2 = service.add_role_for_user(600, 1).await.unwrap();

    // First should return true, second might return false
    assert!(result1);
}
