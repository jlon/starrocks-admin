use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::models::PermissionResponse;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Role {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRoleRequest {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct RoleResponse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

impl From<Role> for RoleResponse {
    fn from(role: Role) -> Self {
        Self {
            id: role.id,
            code: role.code,
            name: role.name,
            description: role.description,
            is_system: role.is_system,
            created_at: role.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleWithPermissions {
    #[serde(flatten)]
    pub role: RoleResponse,
    pub permissions: Vec<PermissionResponse>,
}
