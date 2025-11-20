// Multi-tenant role service tests

use crate::models::{CreateRoleRequest, UpdateRolePermissionsRequest, UpdateRoleRequest};
use crate::services::{permission_service::PermissionService, role_service::RoleService};
use crate::tests::common::{
    create_role, create_test_casbin_service, create_test_db, setup_multi_tenant_test_data,
};
use std::sync::Arc;

#[tokio::test]
async fn test_role_organization_filtering() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service =
        Arc::new(RoleService::new(pool.clone(), casbin_service.clone(), permission_service));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create additional role in org1
    let org1_role_id = create_role(
        &pool,
        "org1_custom_role",
        "Org1 Custom Role",
        "Custom role for organization 1",
        false,
    )
    .await;

    // Update role to belong to org1
    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org1_id)
        .bind(org1_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // Test: Super admin can see all roles
    let all_roles = role_service
        .list_roles(None, true)
        .await
        .expect("Super admin should see all roles");

    assert!(all_roles.len() >= 4); // super_admin, org_admin, regular_user, org1_custom_role

    // Test: Org1 admin can only see org1 roles
    let org1_roles = role_service
        .list_roles(Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should see org1 roles");

    assert!(!org1_roles.is_empty());
    assert!(org1_roles.iter().all(|role| {
        role.organization_id == Some(test_data.org1_id) || role.organization_id.is_none()
    }));

    // Test: Org2 admin cannot see org1 custom role
    let org2_roles = role_service
        .list_roles(Some(test_data.org2_id), false)
        .await
        .expect("Org2 admin should see org2 roles");

    assert!(
        !org2_roles
            .iter()
            .any(|role| role.code == "org1_custom_role")
    );
}

#[tokio::test]
async fn test_role_creation_organization_scoping() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service =
        Arc::new(RoleService::new(pool.clone(), casbin_service.clone(), permission_service));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test: Super admin can create system role (no organization)
    let system_role_req = CreateRoleRequest {
        code: "system_role".to_string(),
        name: "System Role".to_string(),
        description: Some("System-wide role".to_string()),
        organization_id: None,
    };

    let system_role = role_service
        .create_role(system_role_req, None, true)
        .await
        .expect("Super admin should create system role");

    assert_eq!(system_role.code, "system_role");
    assert_eq!(system_role.organization_id, None);

    // Test: Org admin can create organization-scoped role
    let org_role_req = CreateRoleRequest {
        code: "org1_role".to_string(),
        name: "Org1 Role".to_string(),
        description: Some("Organization 1 role".to_string()),
        organization_id: None,
    };

    let org_role = role_service
        .create_role(org_role_req, Some(test_data.org1_id), false)
        .await
        .expect("Org admin should create org-scoped role");

    assert_eq!(org_role.code, "org1_role");
    assert_eq!(org_role.organization_id, Some(test_data.org1_id));

    // Test: Org admin cannot create role without organization context
    let no_org_role_req = CreateRoleRequest {
        code: "no_org_role".to_string(),
        name: "No Org Role".to_string(),
        description: Some("Role without organization".to_string()),
        organization_id: None,
    };

    let result = role_service.create_role(no_org_role_req, None, false).await;

    assert!(result.is_err(), "Org admin should not create role without organization context");
}

#[tokio::test]
async fn test_role_update_organization_validation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service =
        Arc::new(RoleService::new(pool.clone(), casbin_service.clone(), permission_service));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create role in org1
    let org1_role_id =
        create_role(&pool, "org1_test_role", "Org1 Test Role", "Test role in org1", false).await;

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org1_id)
        .bind(org1_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // Test: Org1 admin can update org1 role
    let update_req = UpdateRoleRequest {
        name: Some("Updated Org1 Role".to_string()),
        description: Some("Updated description".to_string()),
        organization_id: None,
    };

    let updated_role = role_service
        .update_role(org1_role_id, update_req, Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should update org1 role");

    assert_eq!(updated_role.name, "Updated Org1 Role");

    // Test: Org2 admin cannot update org1 role
    let update_req = UpdateRoleRequest {
        name: Some("Hijacked Role".to_string()),
        description: Some("This should not work".to_string()),
        organization_id: None,
    };

    let result = role_service
        .update_role(org1_role_id, update_req, Some(test_data.org2_id), false)
        .await;

    assert!(result.is_err(), "Org2 admin should not update org1 role");

    // Test: Super admin can update any role
    let update_req = UpdateRoleRequest {
        name: Some("Super Admin Updated".to_string()),
        description: Some("Updated by super admin".to_string()),
        organization_id: None,
    };

    let updated_role = role_service
        .update_role(org1_role_id, update_req, None, true)
        .await
        .expect("Super admin should update any role");

    assert_eq!(updated_role.name, "Super Admin Updated");
}

#[tokio::test]
async fn test_role_deletion_organization_validation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service =
        Arc::new(RoleService::new(pool.clone(), casbin_service.clone(), permission_service));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create role in org1
    let org1_role_id = create_role(
        &pool,
        "org1_deletable_role",
        "Org1 Deletable Role",
        "Role that can be deleted",
        false,
    )
    .await;

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org1_id)
        .bind(org1_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // Test: Org1 admin can delete org1 role
    role_service
        .delete_role(org1_role_id, Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should delete org1 role");

    // Verify role is deleted
    let result = role_service
        .get_role(org1_role_id, Some(test_data.org1_id), false)
        .await;
    assert!(result.is_err());

    // Create another role for cross-org deletion test
    let org2_role_id = create_role(
        &pool,
        "org2_protected_role",
        "Org2 Protected Role",
        "Role that should be protected from org1",
        false,
    )
    .await;

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org2_id)
        .bind(org2_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // Test: Org1 admin cannot delete org2 role
    let result = role_service
        .delete_role(org2_role_id, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not delete org2 role");

    // Test: Super admin can delete any role
    role_service
        .delete_role(org2_role_id, None, true)
        .await
        .expect("Super admin should delete any role");
}

#[tokio::test]
async fn test_role_permission_assignment_organization_validation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service =
        Arc::new(RoleService::new(pool.clone(), casbin_service.clone(), permission_service));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create permissions for testing
    let permission_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM permissions LIMIT 3")
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch permissions");

    // Create role in org1
    let org1_role_id = create_role(
        &pool,
        "org1_permission_role",
        "Org1 Permission Role",
        "Role for testing permission assignment",
        false,
    )
    .await;

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org1_id)
        .bind(org1_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // Test: Org1 admin can assign permissions to org1 role
    role_service
        .assign_permissions_to_role(
            org1_role_id,
            UpdateRolePermissionsRequest { permission_ids: permission_ids.clone() },
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Org1 admin should assign permissions to org1 role");

    // Verify permissions are assigned
    let assigned_permissions = role_service
        .get_role_permissions(org1_role_id, Some(test_data.org1_id), false)
        .await
        .expect("Should get assigned permissions");

    assert_eq!(assigned_permissions.len(), permission_ids.len());

    // Create role in org2 for cross-org test
    let org2_role_id = create_role(
        &pool,
        "org2_permission_role",
        "Org2 Permission Role",
        "Role for testing cross-org permission assignment",
        false,
    )
    .await;

    sqlx::query("UPDATE roles SET organization_id = ? WHERE id = ?")
        .bind(test_data.org2_id)
        .bind(org2_role_id)
        .execute(&pool)
        .await
        .expect("Failed to assign role to organization");

    // Test: Org1 admin cannot assign permissions to org2 role
    let result = role_service
        .assign_permissions_to_role(
            org2_role_id,
            UpdateRolePermissionsRequest { permission_ids: permission_ids.clone() },
            Some(test_data.org1_id),
            false,
        )
        .await;

    assert!(result.is_err(), "Org1 admin should not assign permissions to org2 role");

    // Test: Super admin can assign permissions to any role
    role_service
        .assign_permissions_to_role(
            org2_role_id,
            UpdateRolePermissionsRequest { permission_ids },
            None,
            true,
        )
        .await
        .expect("Super admin should assign permissions to any role");
}

#[tokio::test]
async fn test_system_role_protection() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service =
        Arc::new(RoleService::new(pool.clone(), casbin_service.clone(), permission_service));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test: System roles cannot be deleted by non-super-admin
    let result = role_service
        .delete_role(test_data.super_admin_role_id, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Non-super-admin should not delete system role");

    // Test: System roles can be updated by super admin
    let update_req = UpdateRoleRequest {
        name: Some("Updated System Role".to_string()),
        description: Some("Updated by super admin".to_string()),
        organization_id: None,
    };

    let result = role_service
        .update_role(test_data.super_admin_role_id, update_req, None, true)
        .await;

    assert!(result.is_err(), "System role name changes should be rejected even for super admin");
}

#[tokio::test]
async fn test_role_organization_isolation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service = Arc::new(PermissionService::new(pool.clone(), casbin_service.clone()));
    let role_service =
        Arc::new(RoleService::new(pool.clone(), casbin_service.clone(), permission_service));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create roles with same display name (codes must remain unique globally)
    let org1_role_id = create_role(
        &pool,
        "duplicate_role_org1",
        "Duplicate Role",
        "Role with duplicate code in org1",
        false,
    )
    .await;

    let org2_role_id = create_role(
        &pool,
        "duplicate_role_org2",
        "Duplicate Role",
        "Role with duplicate code in org2",
        false,
    )
    .await;

    // Assign roles to different organizations
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

    // Test: Org1 admin can only see org1 version of duplicate role
    let org1_roles = role_service
        .list_roles(Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should list org1 roles");

    let org1_duplicate_roles: Vec<_> = org1_roles
        .iter()
        .filter(|role| role.name == "Duplicate Role")
        .collect();

    assert_eq!(org1_duplicate_roles.len(), 1);
    assert_eq!(org1_duplicate_roles[0].organization_id, Some(test_data.org1_id));

    // Test: Org2 admin can only see org2 version of duplicate role
    let org2_roles = role_service
        .list_roles(Some(test_data.org2_id), false)
        .await
        .expect("Org2 admin should list org2 roles");

    let org2_duplicate_roles: Vec<_> = org2_roles
        .iter()
        .filter(|role| role.name == "Duplicate Role")
        .collect();

    assert_eq!(org2_duplicate_roles.len(), 1);
    assert_eq!(org2_duplicate_roles[0].organization_id, Some(test_data.org2_id));

    // Test: Super admin can see both versions
    let all_roles = role_service
        .list_roles(None, true)
        .await
        .expect("Super admin should see all roles");

    let all_duplicate_roles: Vec<_> = all_roles
        .iter()
        .filter(|role| role.name == "Duplicate Role")
        .collect();

    assert_eq!(all_duplicate_roles.len(), 2);
}
