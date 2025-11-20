use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Organization {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrganizationRequest {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    // Option A: create admin user on org creation
    pub admin_username: Option<String>,
    pub admin_password: Option<String>,
    pub admin_email: Option<String>,
    // Option B: assign existing user as admin
    pub admin_user_id: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateOrganizationRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub admin_user_id: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct OrganizationResponse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub admin_user_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl From<Organization> for OrganizationResponse {
    fn from(o: Organization) -> Self {
        Self {
            id: o.id,
            code: o.code,
            name: o.name,
            description: o.description,
            is_system: o.is_system,
            admin_user_id: None, // Will be populated by service layer
            created_at: o.created_at,
        }
    }
}

impl OrganizationResponse {
    pub fn with_admin(mut self, admin_user_id: Option<i64>) -> Self {
        self.admin_user_id = admin_user_id;
        self
    }
}
