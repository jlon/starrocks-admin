use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Permission {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub r#type: String, // type is a Rust keyword, use r#type
    pub resource: Option<String>,
    pub action: Option<String>,
    pub parent_id: Option<i64>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionResponse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub r#type: String,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub parent_id: Option<i64>,
    pub description: Option<String>,
}

impl From<Permission> for PermissionResponse {
    fn from(permission: Permission) -> Self {
        Self {
            id: permission.id,
            code: permission.code,
            name: permission.name,
            r#type: permission.r#type,
            resource: permission.resource,
            action: permission.action,
            parent_id: permission.parent_id,
            description: permission.description,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionTree {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub r#type: String,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    pub children: Vec<PermissionTree>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRolePermissionsRequest {
    pub permission_ids: Vec<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignUserRoleRequest {
    pub role_id: i64,
}
