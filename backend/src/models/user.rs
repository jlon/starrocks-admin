use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::role::RoleResponse;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub organization_id: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminCreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub role_ids: Option<Vec<i64>>,
    pub organization_id: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub current_password: Option<String>,
    pub new_password: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdminUpdateUserRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub password: Option<String>,
    pub role_ids: Option<Vec<i64>>,
    pub organization_id: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub created_at: DateTime<Utc>,
    pub organization_id: Option<i64>,
    pub organization_name: Option<String>,
    pub is_super_admin: bool,
    pub is_org_admin: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserWithRolesResponse {
    #[serde(flatten)]
    pub user: UserResponse,
    pub roles: Vec<RoleResponse>,
}

impl UserResponse {
    pub fn from_user(user: User, is_super_admin: bool, is_org_admin: bool) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            avatar: user.avatar,
            created_at: user.created_at,
            organization_id: user.organization_id,
            organization_name: None,
            is_super_admin,
            is_org_admin,
        }
    }

    pub fn from_user_with_org(
        user: User,
        organization_name: Option<String>,
        is_super_admin: bool,
        is_org_admin: bool,
    ) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            avatar: user.avatar,
            created_at: user.created_at,
            organization_id: user.organization_id,
            organization_name,
            is_super_admin,
            is_org_admin,
        }
    }
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        UserResponse::from_user(user, false, false)
    }
}
