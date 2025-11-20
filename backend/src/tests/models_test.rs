use crate::models::{
    AssignUserRoleRequest, CreateRoleRequest, Permission, PermissionResponse, PermissionTree, Role,
    RoleResponse, UpdateRolePermissionsRequest, UpdateRoleRequest,
};
use chrono::Utc;

#[test]
fn test_permission_response_from_permission() {
    let permission = Permission {
        id: 1,
        code: "menu:dashboard".to_string(),
        name: "Dashboard".to_string(),
        r#type: "menu".to_string(),
        resource: Some("dashboard".to_string()),
        action: None,
        parent_id: None,
        description: Some("Dashboard menu".to_string()),
        created_at: Utc::now(),
    };

    let response: PermissionResponse = permission.into();

    assert_eq!(response.id, 1);
    assert_eq!(response.code, "menu:dashboard");
    assert_eq!(response.name, "Dashboard");
    assert_eq!(response.r#type, "menu");
    assert_eq!(response.resource, Some("dashboard".to_string()));
    assert_eq!(response.action, None);
    assert_eq!(response.parent_id, None);
    assert_eq!(response.description, Some("Dashboard menu".to_string()));
}

#[test]
fn test_role_response_from_role() {
    let role = Role {
        id: 1,
        code: "admin".to_string(),
        name: "Administrator".to_string(),
        description: Some("Admin role".to_string()),
        is_system: true,
        organization_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let response: RoleResponse = role.clone().into();

    assert_eq!(response.id, 1);
    assert_eq!(response.code, "admin");
    assert_eq!(response.name, "Administrator");
    assert_eq!(response.description, Some("Admin role".to_string()));
    assert!(response.is_system);
    assert_eq!(response.created_at, role.created_at);
}

#[test]
fn test_create_role_request_deserialization() {
    // Test deserialization
    let json_str = r#"{"code":"test_role","name":"Test Role","description":"Test description"}"#;
    let deserialized: CreateRoleRequest = serde_json::from_str(json_str).unwrap();
    assert_eq!(deserialized.code, "test_role");
    assert_eq!(deserialized.name, "Test Role");
    assert_eq!(deserialized.description, Some("Test description".to_string()));
    assert_eq!(deserialized.organization_id, None);
}

#[test]
fn test_update_role_request_deserialization() {
    let json_str = r#"{"name":"Updated Name","description":"Updated description"}"#;
    let deserialized: UpdateRoleRequest = serde_json::from_str(json_str).unwrap();
    assert_eq!(deserialized.name, Some("Updated Name".to_string()));
    assert_eq!(deserialized.description, Some("Updated description".to_string()));
    assert_eq!(deserialized.organization_id, None);

    // Test with null values
    let json_str2 = r#"{}"#;
    let deserialized2: UpdateRoleRequest = serde_json::from_str(json_str2).unwrap();
    assert_eq!(deserialized2.name, None);
    assert_eq!(deserialized2.description, None);
    assert_eq!(deserialized2.organization_id, None);
}

#[test]
fn test_assign_user_role_request_deserialization() {
    let json_str = r#"{"role_id":123}"#;
    let deserialized: AssignUserRoleRequest = serde_json::from_str(json_str).unwrap();
    assert_eq!(deserialized.role_id, 123);
}

#[test]
fn test_update_role_permissions_request_deserialization() {
    let json_str = r#"{"permission_ids":[1,2,3,4,5]}"#;
    let deserialized: UpdateRolePermissionsRequest = serde_json::from_str(json_str).unwrap();
    assert_eq!(deserialized.permission_ids, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_update_role_permissions_request_empty() {
    let json_str = r#"{"permission_ids":[]}"#;
    let deserialized: UpdateRolePermissionsRequest = serde_json::from_str(json_str).unwrap();
    assert_eq!(deserialized.permission_ids.len(), 0);
}

#[test]
fn test_permission_tree_structure() {
    let tree = PermissionTree {
        id: 1,
        code: "menu:dashboard".to_string(),
        name: "Dashboard".to_string(),
        r#type: "menu".to_string(),
        resource: Some("dashboard".to_string()),
        action: None,
        description: Some("Dashboard menu".to_string()),
        children: vec![PermissionTree {
            id: 2,
            code: "menu:dashboard:sub".to_string(),
            name: "Dashboard Submenu".to_string(),
            r#type: "menu".to_string(),
            resource: Some("dashboard".to_string()),
            action: None,
            description: None,
            children: vec![],
        }],
    };

    // Test serialization
    let json = serde_json::to_string(&tree);
    assert!(json.is_ok());

    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].id, 2);
}

#[test]
fn test_role_response_serialization() {
    let role = RoleResponse {
        id: 1,
        code: "admin".to_string(),
        name: "Administrator".to_string(),
        description: Some("Admin role".to_string()),
        is_system: true,
        organization_id: None,
        created_at: Utc::now(),
    };

    let json = serde_json::to_string(&role);
    assert!(json.is_ok());
}

#[test]
fn test_permission_response_serialization() {
    let perm = PermissionResponse {
        id: 1,
        code: "menu:dashboard".to_string(),
        name: "Dashboard".to_string(),
        r#type: "menu".to_string(),
        resource: Some("dashboard".to_string()),
        action: None,
        parent_id: None,
        description: Some("Dashboard menu".to_string()),
    };

    let json = serde_json::to_string(&perm);
    assert!(json.is_ok());
}
