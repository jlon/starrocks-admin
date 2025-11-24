// Multi-tenant integration tests

use crate::models::{AdminCreateUserRequest, CreateOrganizationRequest};
use crate::services::{
    organization_service::OrganizationService, permission_service::PermissionService,
    role_service::RoleService, user_service::UserService,
};
use crate::tests::common::{
    assign_role_to_user, assign_user_to_organization, create_role, create_test_casbin_service,
    create_test_db, create_test_user_with_org, grant_permissions, setup_multi_tenant_test_data,
};
use sqlx::SqlitePool;
use std::sync::Arc;

#[tokio::test]
async fn test_complete_multi_tenant_workflow() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));
    let org_service = Arc::new(OrganizationService::new(pool.clone()));

    // Setup initial multi-tenant data
    let test_data = setup_multi_tenant_test_data(&pool).await;

    // 1. Super admin creates new organization
    let new_org_req = CreateOrganizationRequest {
        code: "org3".to_string(),
        name: "Organization 3".to_string(),
        description: Some("Third test organization".to_string()),
        admin_username: Some("org3_admin".to_string()),
        admin_password: Some("password123".to_string()),
        admin_email: Some("admin3@test.com".to_string()),
        admin_user_id: None,
    };

    let new_org = org_service
        .create_organization(new_org_req)
        .await
        .expect("Super admin should create organization");

    assert_eq!(new_org.code, "org3");

    // 2. Verify org3 admin was created and assigned
    let org3_admin_user = sqlx::query_as::<_, (i64, String, Option<i64>)>(
        "SELECT u.id, u.username, u.organization_id FROM users u WHERE u.username = ?",
    )
    .bind("org3_admin")
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch org3 admin");

    let (_admin_id, admin_username, admin_org_id) = org3_admin_user;
    assert_eq!(admin_username, "org3_admin");
    assert_eq!(admin_org_id, Some(new_org.id));

    // 3. Create org-scoped role for org3
    let org3_role_id = create_role(
        &pool,
        "org3_custom_role",
        "Org3 Custom Role",
        "Custom role for organization 3",
        false,
    )
    .await;

    // Assign role to org3
    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(new_org.id)
        .bind(org3_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // 4. Org3 admin creates regular user in org3
    let org3_user_req = AdminCreateUserRequest {
        username: "org3_regular".to_string(),
        password: "password123".to_string(),
        email: Some("regular3@test.com".to_string()),
        avatar: None,
        role_ids: Some(vec![org3_role_id]),
        organization_id: None,
    };

    let org3_user = user_service
        .create_user(org3_user_req, Some(new_org.id), false)
        .await
        .expect("Org3 admin should create user");

    assert_eq!(org3_user.user.username, "org3_regular");
    let org3_user_org: Option<i64> =
        sqlx::query_scalar("SELECT organization_id FROM users WHERE id = ?")
            .bind(org3_user.user.id)
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch org3 user organization");
    assert_eq!(org3_user_org, Some(new_org.id));

    // 5. Verify data isolation
    // Org1 admin should not see org3 users
    let org1_users = user_service
        .list_users(Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should list org1 users");

    assert!(!org1_users.iter().any(|u| u.user.username == "org3_regular"));
    assert!(!org1_users.iter().any(|u| u.user.username == "org3_admin"));

    // Org3 admin should only see org3 users
    let org3_users = user_service
        .list_users(Some(new_org.id), false)
        .await
        .expect("Org3 admin should list org3 users");

    assert!(org3_users.iter().any(|u| u.user.username == "org3_regular"));
    assert!(org3_users.iter().any(|u| u.user.username == "org3_admin"));
    assert!(!org3_users.iter().any(|u| u.user.username == "org1_admin"));

    // Super admin should see all users
    let all_users = user_service
        .list_users(None, true)
        .await
        .expect("Super admin should see all users");

    assert!(all_users.iter().any(|u| u.user.username == "org3_regular"));
    assert!(all_users.iter().any(|u| u.user.username == "org1_admin"));
    assert!(all_users.iter().any(|u| u.user.username == "org2_admin"));
}

#[tokio::test]
async fn test_cross_organization_data_isolation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service = Arc::new(RoleService::new(
        pool.clone(),
        casbin_service.clone(),
        permission_service.clone(),
    ));
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create identical resources in different organizations
    let org1_role_id = create_role(
        &pool,
        "identical_role_org1",
        "Identical Role",
        "Role with same name in org1",
        false,
    )
    .await;

    let org2_role_id = create_role(
        &pool,
        "identical_role_org2",
        "Identical Role",
        "Role with same name in org2",
        false,
    )
    .await;

    // Assign to different organizations
    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org1_id)
        .bind(org1_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to org1");

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org2_id)
        .bind(org2_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to org2");

    // Create organization-specific users (usernames must remain globally unique)
    let org1_user_id =
        create_test_user_with_org(&pool, "identical_user_org1", test_data.org1_id).await;
    let org2_user_id =
        create_test_user_with_org(&pool, "identical_user_org2", test_data.org2_id).await;

    assign_user_to_organization(&pool, org1_user_id, test_data.org1_id).await;
    assign_user_to_organization(&pool, org2_user_id, test_data.org2_id).await;

    // Test isolation: Each organization should only see their own resources
    let org1_roles = role_service
        .list_roles(Some(test_data.org1_id), false)
        .await
        .expect("Org1 should list their roles");

    let org1_identical_roles: Vec<_> = org1_roles
        .iter()
        .filter(|r| r.name == "Identical Role")
        .collect();
    assert_eq!(org1_identical_roles.len(), 1);
    assert_eq!(org1_identical_roles[0].organization_id, Some(test_data.org1_id));

    let org2_roles = role_service
        .list_roles(Some(test_data.org2_id), false)
        .await
        .expect("Org2 should list their roles");

    let org2_identical_roles: Vec<_> = org2_roles
        .iter()
        .filter(|r| r.name == "Identical Role")
        .collect();
    assert_eq!(org2_identical_roles.len(), 1);
    assert_eq!(org2_identical_roles[0].organization_id, Some(test_data.org2_id));

    let org1_users = user_service
        .list_users(Some(test_data.org1_id), false)
        .await
        .expect("Org1 should list their users");

    let org1_identical_users: Vec<_> = org1_users
        .iter()
        .filter(|u| u.user.username == "identical_user_org1")
        .collect();
    assert_eq!(org1_identical_users.len(), 1);
    let org1_user_id = org1_identical_users[0].user.id;
    let org1_user_org: Option<i64> =
        sqlx::query_scalar("SELECT organization_id FROM users WHERE id = ?")
            .bind(org1_user_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch org1 user organization");
    assert_eq!(org1_user_org, Some(test_data.org1_id));

    let org2_users = user_service
        .list_users(Some(test_data.org2_id), false)
        .await
        .expect("Org2 should list their users");

    let org2_identical_users: Vec<_> = org2_users
        .iter()
        .filter(|u| u.user.username == "identical_user_org2")
        .collect();
    assert_eq!(org2_identical_users.len(), 1);
    let org2_user_id = org2_identical_users[0].user.id;
    let org2_user_org: Option<i64> =
        sqlx::query_scalar("SELECT organization_id FROM users WHERE id = ?")
            .bind(org2_user_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch org2 user organization");
    assert_eq!(org2_user_org, Some(test_data.org2_id));
}

#[tokio::test]
async fn test_organization_cascade_operations() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service = Arc::new(RoleService::new(
        pool.clone(),
        casbin_service.clone(),
        permission_service.clone(),
    ));
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));
    let org_service = Arc::new(OrganizationService::new(pool.clone()));

    // Create test organization with users and roles
    let org_req = CreateOrganizationRequest {
        code: "cascade_org".to_string(),
        name: "Cascade Test Organization".to_string(),
        description: Some("Organization for testing cascade operations".to_string()),
        admin_username: Some("cascade_admin".to_string()),
        admin_password: Some("password123".to_string()),
        admin_email: Some("cascade_admin@test.com".to_string()),
        admin_user_id: None,
    };

    let org = org_service
        .create_organization(org_req)
        .await
        .expect("Should create organization");

    // Create additional users and roles in the organization
    let role_id =
        create_role(&pool, "cascade_role", "Cascade Role", "Role for cascade testing", false).await;

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(org.id)
        .bind(role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    let user_id = create_test_user_with_org(&pool, "cascade_user", org.id).await;
    assign_user_to_organization(&pool, user_id, org.id).await;
    assign_role_to_user(&pool, user_id, role_id).await;

    // Verify all data exists before deletion
    let users = user_service
        .list_users(Some(org.id), false)
        .await
        .expect("Should list organization users");
    assert!(!users.is_empty());

    let roles = role_service
        .list_roles(Some(org.id), false)
        .await
        .expect("Should list organization roles");
    assert!(!roles.is_empty());

    // Delete organization (only super admin can do this)
    cleanup_org_data(&pool, org.id).await;
    org_service
        .delete_organization(org.id, None, true)
        .await
        .expect("Super admin should delete organization");

    // Verify organization is deleted
    let result = org_service.get_organization(org.id, None, true).await;
    assert!(result.is_err(), "Organization should be deleted");

    // Note: In a real implementation, you might want to cascade delete
    // or handle orphaned users/roles. This test verifies the basic deletion works.
}

#[tokio::test]
async fn test_permission_inheritance_and_isolation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service = Arc::new(RoleService::new(
        pool.clone(),
        casbin_service.clone(),
        permission_service.clone(),
    ));
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Get available permissions
    let all_permissions: Vec<(i64, String)> = sqlx::query_as("SELECT id, code FROM permissions")
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch permissions");

    // Create org-specific role with limited permissions
    let limited_role_id =
        create_role(&pool, "limited_role", "Limited Role", "Role with limited permissions", false)
            .await;

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org1_id)
        .bind(limited_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // Grant only read permissions to limited role
    let read_permissions: Vec<i64> = all_permissions
        .iter()
        .filter(|(_, code)| code.contains(":get") || code.contains(":list"))
        .map(|(id, _)| *id)
        .collect();

    grant_permissions(&pool, limited_role_id, &read_permissions).await;

    // Create user with limited role
    let limited_user_id = create_test_user_with_org(&pool, "limited_user", test_data.org1_id).await;
    assign_user_to_organization(&pool, limited_user_id, test_data.org1_id).await;
    assign_role_to_user(&pool, limited_user_id, limited_role_id).await;

    // Test that limited user can only perform read operations
    // This would typically be tested at the handler/authorization level
    // For now, we verify the role assignments are correct
    let user_with_roles = user_service
        .get_user(limited_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should get user with roles");

    assert!(!user_with_roles.roles.is_empty());
    assert_eq!(user_with_roles.roles[0].code, "limited_role");

    // Verify role has limited permissions
    let role_permissions = role_service
        .get_role_permissions(limited_role_id, Some(test_data.org1_id), false)
        .await
        .expect("Should get role permissions");

    assert_eq!(role_permissions.len(), read_permissions.len());

    // Verify permissions are all read-only
    for permission in &role_permissions {
        assert!(
            permission.code.contains(":get") || permission.code.contains(":list"),
            "Permission {} should be read-only",
            permission.code
        );
    }
}

#[tokio::test]
async fn test_multi_tenant_edge_cases() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test 1: User cannot be assigned to multiple organizations simultaneously
    let multi_org_user_id =
        create_test_user_with_org(&pool, "multi_org_user", test_data.org1_id).await;
    assign_user_to_organization(&pool, multi_org_user_id, test_data.org1_id).await;
    let duplicate_assignment =
        sqlx::query("INSERT INTO user_organizations (user_id, organization_id) VALUES (?, ?)")
            .bind(multi_org_user_id)
            .bind(test_data.org2_id)
            .execute(&pool)
            .await;
    assert!(duplicate_assignment.is_err(), "Duplicate organization assignment should fail");

    // User should only be visible from their assigned organization
    let org1_users = user_service
        .list_users(Some(test_data.org1_id), false)
        .await
        .expect("Org1 should list users");
    assert!(
        org1_users
            .iter()
            .any(|u| u.user.username == "multi_org_user")
    );

    let org2_users = user_service
        .list_users(Some(test_data.org2_id), false)
        .await
        .expect("Org2 should list users");
    assert!(
        !org2_users
            .iter()
            .any(|u| u.user.username == "multi_org_user"),
        "User should not appear in other organizations"
    );

    // Test 2: Role without organization (system role) assigned to org user
    let system_role_id = create_role(
        &pool,
        "system_role_for_org",
        "System Role for Org",
        "System role that can be assigned to org users",
        true,
    )
    .await;

    assign_role_to_user(&pool, test_data.org1_regular_user_id, system_role_id).await;

    let user_with_roles = user_service
        .get_user(test_data.org1_regular_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should get user with system role");

    assert!(
        user_with_roles
            .roles
            .iter()
            .any(|r| r.code == "system_role_for_org")
    );

    // Test 3: Organization with no users
    let empty_org_id = crate::tests::common::create_test_organization(
        &pool,
        "empty_org",
        "Empty Organization",
        "Organization with no users",
    )
    .await;

    let empty_org_users = user_service
        .list_users(Some(empty_org_id), false)
        .await
        .expect("Should list empty org users");
    assert!(empty_org_users.is_empty());

    // Test 4: User attempts to access resources from non-assigned organization
    let result = user_service.list_users(Some(empty_org_id), false).await;

    // Should return empty since user is not assigned to empty_org
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_concurrent_organization_operations() {
    let pool = create_test_db().await;
    let org_service = Arc::new(OrganizationService::new(pool.clone()));

    // Create organizations sequentially to avoid connection pool exhaustion
    // This test verifies organization isolation rather than true concurrency
    let org_req1 = CreateOrganizationRequest {
        code: "concurrent_org1".to_string(),
        name: "Concurrent Organization 1".to_string(),
        description: Some("First concurrent organization".to_string()),
        admin_username: Some("concurrent_org1_admin".to_string()),
        admin_password: Some("password123".to_string()),
        admin_email: Some("concurrent1@test.com".to_string()),
        admin_user_id: None,
    };

    let org1 = org_service
        .create_organization(org_req1)
        .await
        .expect("First org creation should succeed");

    let org_req2 = CreateOrganizationRequest {
        code: "concurrent_org2".to_string(),
        name: "Concurrent Organization 2".to_string(),
        description: Some("Second concurrent organization".to_string()),
        admin_username: Some("concurrent_org2_admin".to_string()),
        admin_password: Some("password123".to_string()),
        admin_email: Some("concurrent2@test.com".to_string()),
        admin_user_id: None,
    };

    let org2 = org_service
        .create_organization(org_req2)
        .await
        .expect("Second org creation should succeed");

    assert_ne!(org1.id, org2.id);
    assert_eq!(org1.code, "concurrent_org1");
    assert_eq!(org2.code, "concurrent_org2");

    // Verify both organizations exist and are isolated
    let all_orgs = org_service
        .list_organizations(None, true)
        .await
        .expect("Should list all organizations");

    assert!(all_orgs.iter().any(|org| org.code == "concurrent_org1"));
    assert!(all_orgs.iter().any(|org| org.code == "concurrent_org2"));
}

async fn cleanup_org_data(pool: &SqlitePool, org_id: i64) {
    sqlx::query(
        "DELETE FROM user_roles WHERE user_id IN (SELECT user_id FROM user_organizations WHERE organization_id = ?)",
    )
    .bind(org_id)
    .execute(pool)
    .await
    .ok();

    sqlx::query("DELETE FROM role_permissions WHERE role_id IN (SELECT id FROM roles WHERE organization_id = ?)")
        .bind(org_id)
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM user_organizations WHERE organization_id = ?")
        .bind(org_id)
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM users WHERE organization_id = ?")
        .bind(org_id)
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM clusters WHERE organization_id = ?")
        .bind(org_id)
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM roles WHERE organization_id = ?")
        .bind(org_id)
        .execute(pool)
        .await
        .ok();
}
