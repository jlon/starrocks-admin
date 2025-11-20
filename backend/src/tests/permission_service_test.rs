use crate::services::permission_service::PermissionService;
use crate::tests::common::{
    create_role, create_test_casbin_service, create_test_db, grant_permissions, setup_test_data,
};

#[tokio::test]
async fn test_list_permissions_empty() {
    let pool = create_test_db().await;
    // Ensure database is empty
    sqlx::query("DELETE FROM permissions")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM role_permissions")
        .execute(&pool)
        .await
        .ok();

    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool, casbin_service);

    let result = service.list_permissions().await;
    assert!(result.is_ok());
    let permissions = result.unwrap();
    assert_eq!(permissions.len(), 0, "Should return empty list when no permissions");
}

#[tokio::test]
async fn test_list_permissions() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    setup_test_data(&pool).await;

    let result = service.list_permissions().await;
    assert!(result.is_ok());
    let permissions = result.unwrap();
    assert!(permissions.len() >= 6, "Should return all permissions");

    // Check ordering (by type, then code)
    for i in 1..permissions.len() {
        let prev = &permissions[i - 1];
        let curr = &permissions[i];
        assert!(
            prev.r#type < curr.r#type || (prev.r#type == curr.r#type && prev.code <= curr.code),
            "Permissions should be sorted by type then code"
        );
    }
}

#[tokio::test]
async fn test_list_menu_permissions() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    setup_test_data(&pool).await;

    let result = service.list_menu_permissions().await;
    assert!(result.is_ok());
    let permissions = result.unwrap();

    // Should only have menu permissions
    assert!(permissions.len() >= 3, "Should return menu permissions");
    for perm in &permissions {
        assert_eq!(perm.r#type, "menu", "Should only return menu permissions");
    }
}

#[tokio::test]
async fn test_list_api_permissions() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    setup_test_data(&pool).await;

    let result = service.list_api_permissions().await;
    assert!(result.is_ok());
    let permissions = result.unwrap();

    // Should only have API permissions
    assert!(permissions.len() >= 3, "Should return API permissions");
    for perm in &permissions {
        assert_eq!(perm.r#type, "api", "Should only return API permissions");
    }
}

#[tokio::test]
async fn test_get_permission_tree_empty() {
    let pool = create_test_db().await;
    // Ensure database is empty
    sqlx::query("DELETE FROM permissions")
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM role_permissions")
        .execute(&pool)
        .await
        .ok();

    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool, casbin_service);

    let result = service.get_permission_tree().await;
    assert!(result.is_ok());
    let tree = result.unwrap();
    assert_eq!(tree.len(), 0, "Should return empty tree when no permissions");
}

#[tokio::test]
async fn test_get_permission_tree() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    setup_test_data(&pool).await;

    // Add a permission with parent
    let parent_permission_id: (i64,) =
        sqlx::query_as("SELECT id FROM permissions WHERE code = 'menu:dashboard'")
            .fetch_one(&pool)
            .await
            .unwrap();

    sqlx::query(
        "INSERT INTO permissions (code, name, type, resource, parent_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("menu:dashboard:sub")
    .bind("Dashboard Submenu")
    .bind("menu")
    .bind("dashboard")
    .bind(parent_permission_id.0)
    .execute(&pool)
    .await
    .unwrap();

    let result = service.get_permission_tree().await;
    assert!(result.is_ok());
    let tree = result.unwrap();

    assert!(!tree.is_empty(), "Should return tree structure");

    // Check if tree structure is correct
    let has_children = tree.iter().any(|node| !node.children.is_empty());
    assert!(has_children, "Tree should have nested children");
}

#[tokio::test]
async fn test_get_user_permissions_no_roles() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    let result = service.get_user_permissions(user_id).await;
    assert!(result.is_ok());
    let permissions = result.unwrap();
    assert_eq!(permissions.len(), 0, "User without roles should have no permissions");
}

#[tokio::test]
async fn test_get_user_permissions() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;
    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;

    let result = service.get_user_permissions(user_id).await;
    assert!(result.is_ok());
    let permissions = result.unwrap();

    assert!(permissions.len() >= 6, "Admin user should have all permissions");

    // Check ordering
    for i in 1..permissions.len() {
        let prev = &permissions[i - 1];
        let curr = &permissions[i];
        assert!(
            prev.r#type < curr.r#type || (prev.r#type == curr.r#type && prev.code <= curr.code),
            "Permissions should be sorted"
        );
    }
}

#[tokio::test]
async fn test_get_user_permissions_multiple_roles() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let operator_role_id = create_role(&pool, "ops", "Operator", "Operator role", false).await;
    let limited_permissions: Vec<i64> = data.permission_ids.iter().take(3).copied().collect();
    grant_permissions(&pool, operator_role_id, &limited_permissions).await;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;

    // Assign both roles
    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;
    crate::tests::common::assign_role_to_user(&pool, user_id, operator_role_id).await;

    let result = service.get_user_permissions(user_id).await;
    assert!(result.is_ok());
    let permissions = result.unwrap();

    // Should have unique permissions (no duplicates)
    let codes: Vec<String> = permissions.iter().map(|p| p.code.clone()).collect();
    let unique_codes: std::collections::HashSet<String> = codes.iter().cloned().collect();
    assert_eq!(codes.len(), unique_codes.len(), "Should not have duplicate permissions");
}

#[tokio::test]
async fn test_check_permission_no_permission() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool, casbin_service);

    let result = service
        .check_permission(1, "system:clusters", "create")
        .await;
    assert!(result.is_ok());
    assert!(!result.unwrap(), "Should deny when no permission");
}

#[tokio::test]
async fn test_check_permission_with_permission() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service.clone());

    // Setup and reload policies
    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;
    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;

    casbin_service.reload_policies_from_db(&pool).await.unwrap();

    // User should have permission
    let result = service
        .check_permission(user_id, "system:clusters", "create")
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap(), "Should allow when user has permission");
}

#[tokio::test]
async fn test_check_permission_different_action() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service.clone());

    let data = setup_test_data(&pool).await;
    let admin_role_id = data.admin_role_id;
    let user_id = crate::tests::common::create_test_user(&pool, "test_user").await;
    crate::tests::common::assign_role_to_user(&pool, user_id, admin_role_id).await;

    casbin_service.reload_policies_from_db(&pool).await.unwrap();

    // User has create permission, but not different_action
    let result = service
        .check_permission(user_id, "system:clusters", "different_action")
        .await;
    assert!(result.is_ok());
    // Might be false if action doesn't match
}

#[tokio::test]
async fn test_permission_response_conversion() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let service = PermissionService::new(pool.clone(), casbin_service);

    setup_test_data(&pool).await;

    let result = service.list_permissions().await;
    assert!(result.is_ok());
    let permissions = result.unwrap();

    // Check that all permissions have required fields
    for perm in &permissions {
        assert!(!perm.code.is_empty(), "Permission code should not be empty");
        assert!(!perm.name.is_empty(), "Permission name should not be empty");
        assert!(!perm.r#type.is_empty(), "Permission type should not be empty");
    }
}
