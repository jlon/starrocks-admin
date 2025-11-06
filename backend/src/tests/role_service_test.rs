use crate::models::{CreateRoleRequest, UpdateRolePermissionsRequest, UpdateRoleRequest};
use crate::services::role_service::RoleService;
use crate::services::permission_service::PermissionService;
use crate::tests::common::{
    create_role,
    create_test_casbin_service,
    create_test_db,
    setup_test_data,
};
use crate::utils::ApiError;
use std::sync::Arc;

async fn create_test_role_service() -> RoleService {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    RoleService::new(pool, casbin_service, permission_service)
}

#[tokio::test]
async fn test_list_roles_empty() {
    let pool = create_test_db().await;
    // Ensure database is empty
    sqlx::query("DELETE FROM user_roles")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM role_permissions")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM roles").execute(&pool).await.ok();

    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool, casbin_service, permission_service);

    let result = service.list_roles().await;
    assert!(result.is_ok());
    let roles = result.unwrap();
    assert_eq!(roles.len(), 0, "Should return empty list when no roles");
}

#[tokio::test]
async fn test_list_roles() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    setup_test_data(&pool).await;
    create_role(&pool, "ops", "Ops", "Ops role", false).await;
    create_role(&pool, "auditor", "Auditor", "Auditor role", false).await;

    let result = service.list_roles().await;
    assert!(result.is_ok());
    let roles = result.unwrap();
    assert!(roles.len() >= 3, "Should return all roles");

    // System roles should come first
    assert!(roles[0].is_system, "System roles should be first");
}

#[tokio::test]
async fn test_get_role_not_found() {
    let service = create_test_role_service().await;

    let result = service.get_role(999).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ApiError::SystemFunctionNotFound(_) => {}, // ApiError::not_found returns SystemFunctionNotFound
        _ => panic!("Should return not found error"),
    }
}

#[tokio::test]
async fn test_get_role() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;

    let result = service.get_role(admin_role_id).await;
    assert!(result.is_ok());
    let role = result.unwrap();
    assert_eq!(role.id, admin_role_id);
    assert_eq!(role.code, "admin");
    assert!(role.is_system);
}

#[tokio::test]
async fn test_get_role_with_permissions() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;

    let result = service.get_role_with_permissions(admin_role_id).await;
    assert!(result.is_ok());
    let role_with_perms = result.unwrap();
    assert_eq!(role_with_perms.role.id, admin_role_id);
    assert!(role_with_perms.permissions.len() >= 6, "Admin should have all permissions");
}

#[tokio::test]
async fn test_create_role() {
    let service = create_test_role_service().await;

    let req = CreateRoleRequest {
        code: "test_role".to_string(),
        name: "Test Role".to_string(),
        description: Some("Test description".to_string()),
    };

    let result = service.create_role(req).await;
    assert!(result.is_ok());
    let role = result.unwrap();
    assert_eq!(role.code, "test_role");
    assert_eq!(role.name, "Test Role");
    assert_eq!(role.description, Some("Test description".to_string()));
    assert!(!role.is_system);
}

#[tokio::test]
async fn test_create_role_duplicate_code() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    setup_test_data(&pool).await; // Creates admin

    let req = CreateRoleRequest {
        code: "admin".to_string(), // Duplicate
        name: "Another Admin".to_string(),
        description: None,
    };

    let result = service.create_role(req).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ApiError::ValidationError(_) => {},
        _ => panic!("Should return validation error"),
    }
}

#[tokio::test]
async fn test_update_role() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let _data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;

    let req = UpdateRoleRequest {
        name: Some("Updated Operator".to_string()),
        description: Some("Updated description".to_string()),
    };

    let result = service.update_role(operator_role_id, req).await;
    assert!(result.is_ok());
    let role = result.unwrap();
    assert_eq!(role.name, "Updated Operator");
    assert_eq!(role.description, Some("Updated description".to_string()));
}

#[tokio::test]
async fn test_update_role_name_only() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let _data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;

    let req = UpdateRoleRequest { name: Some("New Name".to_string()), description: None };

    let result = service.update_role(operator_role_id, req).await;
    assert!(result.is_ok());
    let role = result.unwrap();
    assert_eq!(role.name, "New Name");
}

#[tokio::test]
async fn test_update_role_description_only() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let _data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;

    let req = UpdateRoleRequest { name: None, description: Some("Only description".to_string()) };

    let result = service.update_role(operator_role_id, req).await;
    assert!(result.is_ok());
    let role = result.unwrap();
    assert_eq!(role.description, Some("Only description".to_string()));
}

#[tokio::test]
async fn test_update_role_no_changes() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let _data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;

    let req = UpdateRoleRequest { name: None, description: None };

    let result = service.update_role(operator_role_id, req).await;
    assert!(result.is_ok(), "Should return role unchanged");
}

#[tokio::test]
async fn test_update_system_role_name() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;

    let req = UpdateRoleRequest { name: Some("New Admin Name".to_string()), description: None };

    let result = service.update_role(admin_role_id, req).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ApiError::ValidationError(_) => {},
        _ => panic!("Should return validation error for system role name update"),
    }
}

#[tokio::test]
async fn test_update_system_role_description() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;

    let req = UpdateRoleRequest { name: None, description: Some("New description".to_string()) };

    let result = service.update_role(admin_role_id, req).await;
    assert!(result.is_ok(), "Should allow description update for system role");
}

#[tokio::test]
async fn test_delete_role() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let _data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;

    let result = service.delete_role(operator_role_id).await;
    assert!(result.is_ok(), "Should delete non-system role");

    // Verify role is deleted
    let result = service.get_role(operator_role_id).await;
    assert!(result.is_err(), "Role should not exist after deletion");
}

#[tokio::test]
async fn test_delete_system_role() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;

    let result = service.delete_role(admin_role_id).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ApiError::ValidationError(_) => {},
        _ => panic!("Should return validation error for system role deletion"),
    }
}

#[tokio::test]
async fn test_assign_permissions_to_role() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;
    let permission_ids = data.permission_ids.clone();

    // Assign first 3 permissions
    let req = UpdateRolePermissionsRequest { permission_ids: permission_ids[0..3].to_vec() };

    let result = service
        .assign_permissions_to_role(operator_role_id, req)
        .await;
    assert!(result.is_ok(), "Should assign permissions");

    // Verify permissions are assigned
    let role_with_perms = service
        .get_role_with_permissions(operator_role_id)
        .await
        .unwrap();
    assert_eq!(role_with_perms.permissions.len(), 3);
}

#[tokio::test]
async fn test_assign_permissions_to_role_replace() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;
    let permission_ids = data.permission_ids.clone();

    // First assignment
    let req1 = UpdateRolePermissionsRequest { permission_ids: permission_ids[0..3].to_vec() };
    service
        .assign_permissions_to_role(operator_role_id, req1)
        .await
        .unwrap();

    // Second assignment (should replace)
    let req2 = UpdateRolePermissionsRequest { permission_ids: permission_ids[3..6].to_vec() };
    let result = service
        .assign_permissions_to_role(operator_role_id, req2)
        .await;
    assert!(result.is_ok());

    // Verify old permissions are replaced
    let role_with_perms = service
        .get_role_with_permissions(operator_role_id)
        .await
        .unwrap();
    assert_eq!(role_with_perms.permissions.len(), 3);
    assert!(
        role_with_perms
            .permissions
            .iter()
            .all(|p| permission_ids[3..6].contains(&p.id))
    );
}

#[tokio::test]
async fn test_assign_permissions_to_role_empty() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let _data = setup_test_data(&pool).await;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;

    let req = UpdateRolePermissionsRequest { permission_ids: vec![] };

    let result = service
        .assign_permissions_to_role(operator_role_id, req)
        .await;
    assert!(result.is_ok(), "Should allow empty permissions");

    let role_with_perms = service
        .get_role_with_permissions(operator_role_id)
        .await
        .unwrap();
    assert_eq!(role_with_perms.permissions.len(), 0);
}

#[tokio::test]
async fn test_get_role_permissions() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let permission_service =
        Arc::new(PermissionService::new(pool.clone(), Arc::clone(&casbin_service)));
    let service = RoleService::new(pool.clone(), casbin_service, permission_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;

    let result = service.get_role_permissions(admin_role_id).await;
    assert!(result.is_ok());
    let permissions = result.unwrap();
    assert!(permissions.len() >= 6, "Admin should have all permissions");
}
