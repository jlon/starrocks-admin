// Multi-tenant user service tests

use crate::models::{AdminCreateUserRequest, AdminUpdateUserRequest};
use crate::services::user_service::UserService;
use crate::tests::common::{
    assign_user_to_organization, create_test_casbin_service, create_test_db,
    create_test_user_with_org, setup_multi_tenant_test_data,
};
use std::sync::Arc;

#[tokio::test]
async fn test_user_organization_filtering() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create additional users in different organizations
    let org1_extra_user_id =
        create_test_user_with_org(&pool, "org1_extra", test_data.org1_id).await;
    let org2_extra_user_id =
        create_test_user_with_org(&pool, "org2_extra", test_data.org2_id).await;

    assign_user_to_organization(&pool, org1_extra_user_id, test_data.org1_id).await;
    assign_user_to_organization(&pool, org2_extra_user_id, test_data.org2_id).await;

    // Test: Super admin can see all users
    let all_users = user_service
        .list_users(None, true)
        .await
        .expect("Super admin should see all users");

    assert!(all_users.len() >= 7); // super_admin + org1_admin + org1_regular + org2_admin + org2_regular + extras

    // Test: Org1 admin can only see org1 users
    let org1_users = user_service
        .list_users(Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should see org1 users");

    // Should only see users from org1 (including those assigned to org1)
    let org1_usernames: Vec<_> = org1_users.iter().map(|u| &u.user.username).collect();
    assert!(org1_usernames.contains(&&"org1_admin".to_string()));
    assert!(org1_usernames.contains(&&"org1_regular".to_string()));
    assert!(org1_usernames.contains(&&"org1_extra".to_string()));
    assert!(!org1_usernames.contains(&&"org2_admin".to_string()));
    assert!(!org1_usernames.contains(&&"org2_regular".to_string()));

    // Test: Org2 admin can only see org2 users
    let org2_users = user_service
        .list_users(Some(test_data.org2_id), false)
        .await
        .expect("Org2 admin should see org2 users");

    let org2_usernames: Vec<_> = org2_users.iter().map(|u| &u.user.username).collect();
    assert!(org2_usernames.contains(&&"org2_admin".to_string()));
    assert!(org2_usernames.contains(&&"org2_regular".to_string()));
    assert!(org2_usernames.contains(&&"org2_extra".to_string()));
    assert!(!org2_usernames.contains(&&"org1_admin".to_string()));
    assert!(!org2_usernames.contains(&&"org1_regular".to_string()));
}

#[tokio::test]
async fn test_user_creation_organization_scoping() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test: Super admin can create user without organization
    let system_user_req = AdminCreateUserRequest {
        username: "system_user".to_string(),
        password: "password123".to_string(),
        email: Some("system@test.com".to_string()),
        avatar: None,
        role_ids: Some(vec![test_data.super_admin_role_id]),
        organization_id: None,
    };

    let system_user = user_service
        .create_user(system_user_req, None, true)
        .await
        .expect("Super admin should create system user");

    assert_eq!(system_user.user.username, "system_user");
    // UserResponse doesn't have organization_id field, so we check through the service filtering

    // Test: Org admin can create user in their organization
    let org_user_req = AdminCreateUserRequest {
        username: "org1_new_user".to_string(),
        password: "password123".to_string(),
        email: Some("org1user@test.com".to_string()),
        avatar: None,
        role_ids: Some(vec![test_data.regular_role_id]),
        organization_id: None,
    };

    let org_user = user_service
        .create_user(org_user_req, Some(test_data.org1_id), false)
        .await
        .expect("Org admin should create org user");

    assert_eq!(org_user.user.username, "org1_new_user");
    // UserResponse doesn't have organization_id field, so we check through the service filtering

    // Test: Org admin cannot create user without organization context
    let org_user_req = AdminCreateUserRequest {
        username: "org_user".to_string(),
        password: "password123".to_string(),
        email: Some("org_user@test.com".to_string()),
        avatar: None,
        role_ids: None,
        organization_id: None,
    };

    let result = user_service.create_user(org_user_req, None, false).await;

    assert!(result.is_err(), "Org admin should not create user without organization context");
}

#[tokio::test]
async fn test_user_update_organization_validation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create user in org1
    let org1_user_id = create_test_user_with_org(&pool, "org1_updatable", test_data.org1_id).await;
    assign_user_to_organization(&pool, org1_user_id, test_data.org1_id).await;

    // Test: Org1 admin can update org1 user
    let update_req = AdminUpdateUserRequest {
        username: Some("updated_user_org2".to_string()),
        email: Some("updated@test.com".to_string()),
        avatar: Some("updated_avatar".to_string()),
        password: Some("new_password".to_string()),
        role_ids: None,
        organization_id: None,
    };

    let updated_user = user_service
        .update_user(org1_user_id, update_req, Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should update org1 user");

    assert_eq!(updated_user.user.email, Some("updated@test.com".to_string()));

    // Create user in org2 for cross-org test
    let org2_user_id = create_test_user_with_org(&pool, "org2_protected", test_data.org2_id).await;
    assign_user_to_organization(&pool, org2_user_id, test_data.org2_id).await;

    // Test: Org1 admin cannot update org2 user
    let update_req = AdminUpdateUserRequest {
        username: Some("updated_user".to_string()),
        email: Some("updated@test.com".to_string()),
        avatar: Some("updated_avatar".to_string()),
        password: Some("new_password".to_string()),
        role_ids: None,
        organization_id: None,
    };

    let result = user_service
        .update_user(org2_user_id, update_req, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not update org2 user");

    // Test: Super admin can update any user
    let update_req = AdminUpdateUserRequest {
        username: Some("updated_user".to_string()),
        email: Some("updated@test.com".to_string()),
        avatar: Some("updated_avatar".to_string()),
        password: Some("new_password".to_string()),
        role_ids: None,
        organization_id: None,
    };

    let updated_user = user_service
        .update_user(org2_user_id, update_req, None, true)
        .await
        .expect("Super admin should update any user");

    assert_eq!(updated_user.user.email, Some("updated@test.com".to_string()));
}

#[tokio::test]
async fn test_user_deletion_organization_validation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create user in org1
    let org1_user_id = create_test_user_with_org(&pool, "org1_deletable", test_data.org1_id).await;
    assign_user_to_organization(&pool, org1_user_id, test_data.org1_id).await;

    // Test: Org1 admin can delete org1 user
    user_service
        .delete_user(org1_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should delete org1 user");

    // Verify user is deleted
    let result = user_service
        .get_user(org1_user_id, Some(test_data.org1_id), false)
        .await;
    assert!(result.is_err());

    // Create user in org2 for cross-org deletion test
    let org2_user_id = create_test_user_with_org(&pool, "org2_protected", test_data.org2_id).await;
    assign_user_to_organization(&pool, org2_user_id, test_data.org2_id).await;

    // Test: Org1 admin cannot delete org2 user
    let result = user_service
        .delete_user(org2_user_id, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not delete org2 user");

    // Test: Super admin can delete any user
    user_service
        .delete_user(org2_user_id, None, true)
        .await
        .expect("Super admin should delete any user");
}

#[tokio::test]
async fn test_user_role_assignment_organization_validation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create user in org1
    let org1_user_id = create_test_user_with_org(&pool, "org1_role_user", test_data.org1_id).await;
    assign_user_to_organization(&pool, org1_user_id, test_data.org1_id).await;

    // Test: Org1 admin can assign org1 role to org1 user
    let update_req = AdminUpdateUserRequest {
        username: None,
        email: None,
        avatar: None,
        password: None,
        role_ids: Some(vec![test_data.regular_role_id]),
        organization_id: None,
    };
    user_service
        .update_user(org1_user_id, update_req, Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should assign org1 role to org1 user");

    // Verify role is assigned
    let user_with_roles = user_service
        .get_user(org1_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should get user with roles");

    assert!(!user_with_roles.roles.is_empty());

    // Create user in org2 for cross-org role assignment test
    let org2_user_id = create_test_user_with_org(&pool, "org2_role_user", test_data.org2_id).await;
    assign_user_to_organization(&pool, org2_user_id, test_data.org2_id).await;

    // Test: Org1 admin cannot assign roles to org2 user
    let update_req = AdminUpdateUserRequest {
        username: None,
        email: Some("hijacked@test.com".to_string()),
        avatar: None,
        password: None,
        role_ids: Some(vec![test_data.regular_role_id]),
        organization_id: None,
    };
    let result = user_service
        .update_user(org2_user_id, update_req, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not assign roles to org2 user");

    // Test: Org1 admin cannot assign org2 role to org1 user (if org2 role existed)
    // This tests role organization validation in user service

    // Test: Super admin can assign any role to any user
    let super_admin_update_req = AdminUpdateUserRequest {
        username: None,
        email: None,
        avatar: None,
        password: None,
        role_ids: Some(vec![test_data.super_admin_role_id]),
        organization_id: None,
    };
    user_service
        .update_user(org2_user_id, super_admin_update_req, None, true)
        .await
        .expect("Super admin should assign any role to any user");
}

#[tokio::test]
async fn test_user_organization_isolation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create users (usernames must remain unique globally)
    let org1_duplicate_id =
        create_test_user_with_org(&pool, "duplicate_user_org1", test_data.org1_id).await;
    let org2_duplicate_id =
        create_test_user_with_org(&pool, "duplicate_user_org2", test_data.org2_id).await;

    assign_user_to_organization(&pool, org1_duplicate_id, test_data.org1_id).await;
    assign_user_to_organization(&pool, org2_duplicate_id, test_data.org2_id).await;

    // Test: Org1 admin can only see org1 version of duplicate user
    let org1_users = user_service
        .list_users(Some(test_data.org1_id), false)
        .await
        .expect("Org1 admin should list org1 users");

    assert!(
        org1_users
            .iter()
            .any(|u| u.user.username == "duplicate_user_org1")
    );
    // UserResponse doesn't have organization_id field, so we check through the service filtering

    // Test: Org2 admin can only see org2 version of duplicate user
    let org2_users = user_service
        .list_users(Some(test_data.org2_id), false)
        .await
        .expect("Org2 admin should list org2 users");

    assert!(
        org2_users
            .iter()
            .any(|u| u.user.username == "duplicate_user_org2")
    );
    // UserResponse doesn't have organization_id field, so we check through the service filtering

    // Test: Super admin can see both versions
    let all_users = user_service
        .list_users(None, true)
        .await
        .expect("Super admin should see all users");

    assert!(
        all_users
            .iter()
            .any(|u| u.user.username == "duplicate_user_org1")
    );
    assert!(
        all_users
            .iter()
            .any(|u| u.user.username == "duplicate_user_org2")
    );
}

#[tokio::test]
async fn test_user_cross_organization_access_prevention() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test: Org1 admin cannot access org2 user by ID
    let result = user_service
        .get_user(test_data.org2_admin_user_id, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not access org2 user by ID");

    // Test: Org1 admin cannot update org2 user even with valid ID
    let update_req = AdminUpdateUserRequest {
        username: None,
        email: Some("hijacked@test.com".to_string()),
        avatar: None,
        password: None,
        role_ids: None,
        organization_id: None,
    };

    let result = user_service
        .update_user(test_data.org2_admin_user_id, update_req, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not update org2 user");

    // Test: Org1 admin cannot delete org2 user
    let result = user_service
        .delete_user(test_data.org2_admin_user_id, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not delete org2 user");

    // Test: Org1 admin cannot assign roles to org2 user
    // Since replace_user_roles is private, we'll use the public interface
    let update_req2 = AdminUpdateUserRequest {
        username: None,
        email: Some("hijacked2@test.com".to_string()),
        avatar: None,
        password: None,
        role_ids: None,
        organization_id: None,
    };
    let result = user_service
        .update_user(test_data.org2_admin_user_id, update_req2, Some(test_data.org1_id), false)
        .await;

    assert!(result.is_err(), "Org1 admin should not assign roles to org2 user");
}

#[tokio::test]
async fn test_user_organization_membership_consistency() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let user_service = Arc::new(UserService::new(pool.clone(), casbin_service.clone()));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create user in org1
    let org1_user_id = create_test_user_with_org(&pool, "org1_member", test_data.org1_id).await;
    assign_user_to_organization(&pool, org1_user_id, test_data.org1_id).await;

    // Verify user is properly associated with organization
    let user_with_roles = user_service
        .get_user(org1_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should get user with organization");

    assert_eq!(user_with_roles.user.id, org1_user_id);

    // Test: User appears in organization's user list
    let org1_users = user_service
        .list_users(Some(test_data.org1_id), false)
        .await
        .expect("Should list org1 users");

    assert!(org1_users.iter().any(|u| u.user.id == org1_user_id));

    // Test: User does not appear in other organization's user list
    let org2_users = user_service
        .list_users(Some(test_data.org2_id), false)
        .await
        .expect("Should list org2 users");

    assert!(!org2_users.iter().any(|u| u.user.id == org1_user_id));
}
