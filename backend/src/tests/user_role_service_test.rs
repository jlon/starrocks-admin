use crate::models::AssignUserRoleRequest;
use crate::services::user_role_service::UserRoleService;
use crate::tests::common::{
    create_role, create_test_casbin_service, create_test_db, setup_test_data,
};
use crate::utils::ApiError;

#[tokio::test]
async fn test_get_user_roles_no_roles() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    let result = service.get_user_roles(user_id).await;
    assert!(result.is_ok());
    let roles = result.unwrap();
    assert_eq!(roles.len(), 0, "User without roles should return empty");
}

#[tokio::test]
async fn test_get_user_roles() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;
    crate::tests::common::assign_role_to_user(&pool, user_id, operator_role_id).await;

    let result = service.get_user_roles(user_id).await;
    assert!(result.is_ok());
    let roles = result.unwrap();
    assert_eq!(roles.len(), 2, "User should have 2 roles");
}

#[tokio::test]
async fn test_assign_role_to_user() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    let req = AssignUserRoleRequest { role_id: admin_role_id };

    let result = service.assign_role_to_user(user_id, req).await;
    assert!(result.is_ok(), "Should assign role to user");

    // Verify assignment
    let roles = service.get_user_roles(user_id).await.unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].id, admin_role_id);
}

#[tokio::test]
async fn test_assign_role_to_user_duplicate() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    let req = AssignUserRoleRequest { role_id: admin_role_id };

    // First assignment
    service.assign_role_to_user(user_id, req).await.unwrap();

    // Duplicate assignment
    let req2 = AssignUserRoleRequest { role_id: admin_role_id };
    let result = service.assign_role_to_user(user_id, req2).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ApiError::ValidationError(_) => {},
        _ => panic!("Should return validation error for duplicate assignment"),
    }
}

#[tokio::test]
async fn test_assign_role_to_user_role_not_found() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    let req = AssignUserRoleRequest {
        role_id: 9999, // Non-existent role
    };

    let result = service.assign_role_to_user(user_id, req).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ApiError::SystemFunctionNotFound(_) | ApiError::ResourceNotFound(_) => {},
        _ => panic!("Should return not found error"),
    }
}

#[tokio::test]
async fn test_remove_role_from_user() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;
    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;

    let result = service.remove_role_from_user(user_id, admin_role_id).await;
    assert!(result.is_ok(), "Should remove role from user");

    // Verify removal
    let roles = service.get_user_roles(user_id).await.unwrap();
    assert_eq!(roles.len(), 0);
}

#[tokio::test]
async fn test_remove_role_from_user_not_assigned() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    let result = service.remove_role_from_user(user_id, admin_role_id).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ApiError::SystemFunctionNotFound(_) | ApiError::ResourceNotFound(_) => {},
        _ => panic!("Should return not found error"),
    }
}

#[tokio::test]
async fn test_get_user_roles_detailed() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;
    crate::tests::common::assign_role_to_user(&pool, user_id, operator_role_id).await;

    let result = service.get_user_roles_detailed(user_id).await;
    assert!(result.is_ok());
    let roles = result.unwrap();
    assert_eq!(roles.len(), 2);

    // Check that roles have all fields
    for role in &roles {
        assert!(!role.code.is_empty());
        assert!(!role.name.is_empty());
    }
}

#[tokio::test]
async fn test_assign_remove_multiple_roles() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    // Assign first role
    let req1 = AssignUserRoleRequest { role_id: admin_role_id };
    service.assign_role_to_user(user_id, req1).await.unwrap();

    // Assign second role
    let req2 = AssignUserRoleRequest { role_id: operator_role_id };
    service.assign_role_to_user(user_id, req2).await.unwrap();

    // Verify both roles
    let roles = service.get_user_roles(user_id).await.unwrap();
    assert_eq!(roles.len(), 2);

    // Remove first role
    service
        .remove_role_from_user(user_id, admin_role_id)
        .await
        .unwrap();

    // Verify only second role remains
    let roles = service.get_user_roles(user_id).await.unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].id, operator_role_id);
}

#[tokio::test]
async fn test_user_roles_sorted() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = UserRoleService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    // Assign in reverse order
    let req1 = AssignUserRoleRequest { role_id: operator_role_id };
    service.assign_role_to_user(user_id, req1).await.unwrap();

    let req2 = AssignUserRoleRequest { role_id: admin_role_id };
    service.assign_role_to_user(user_id, req2).await.unwrap();

    // Verify roles are sorted by name
    let roles = service.get_user_roles(user_id).await.unwrap();
    assert_eq!(roles.len(), 2);
    // admin should come before ops alphabetically
    assert_eq!(roles[0].code, "admin");
    assert_eq!(roles[1].code, "ops");
}
