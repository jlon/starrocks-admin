use crate::models::{AssignUserRoleRequest, Role, RoleResponse};
use crate::services::casbin_service::CasbinService;
use crate::utils::{ApiError, ApiResult};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct UserRoleService {
    pool: SqlitePool,
    casbin_service: Arc<CasbinService>,
}

impl UserRoleService {
    pub fn new(pool: SqlitePool, casbin_service: Arc<CasbinService>) -> Self {
        Self { pool, casbin_service }
    }

    /// Get user's roles
    pub async fn get_user_roles(&self, user_id: i64) -> ApiResult<Vec<RoleResponse>> {
        let roles: Vec<Role> = sqlx::query_as(
            r#"
            SELECT r.*
            FROM roles r
            JOIN user_roles ur ON r.id = ur.role_id
            WHERE ur.user_id = ?
            ORDER BY r.name
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(roles.into_iter().map(|r| r.into()).collect())
    }

    /// Assign role to user
    pub async fn assign_role_to_user(
        &self,
        user_id: i64,
        req: AssignUserRoleRequest,
    ) -> ApiResult<()> {
        // Check if role exists
        let role: Role = sqlx::query_as("SELECT * FROM roles WHERE id = ?")
            .bind(req.role_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Role not found"))?;

        // Check if user-role assignment already exists
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM user_roles WHERE user_id = ? AND role_id = ?")
                .bind(user_id)
                .bind(req.role_id)
                .fetch_optional(&self.pool)
                .await?;

        if existing.is_some() {
            return Err(ApiError::validation_error("User already has this role"));
        }

        // Insert user-role assignment
        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(req.role_id)
            .execute(&self.pool)
            .await?;

        // Update Casbin
        self.casbin_service
            .add_role_for_user(user_id, req.role_id)
            .await?;

        tracing::info!("Role {} assigned to user {}", role.name, user_id);

        Ok(())
    }

    /// Remove role from user
    pub async fn remove_role_from_user(&self, user_id: i64, role_id: i64) -> ApiResult<()> {
        // Check if assignment exists
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM user_roles WHERE user_id = ? AND role_id = ?")
                .bind(user_id)
                .bind(role_id)
                .fetch_optional(&self.pool)
                .await?;

        if existing.is_none() {
            return Err(ApiError::not_found("User role assignment not found"));
        }

        // Delete user-role assignment
        sqlx::query("DELETE FROM user_roles WHERE user_id = ? AND role_id = ?")
            .bind(user_id)
            .bind(role_id)
            .execute(&self.pool)
            .await?;

        // Update Casbin
        self.casbin_service
            .remove_role_for_user(user_id, role_id)
            .await?;

        tracing::info!("Role {} removed from user {}", role_id, user_id);

        Ok(())
    }

    /// Get all roles for a user (including role details)
    pub async fn get_user_roles_detailed(&self, user_id: i64) -> ApiResult<Vec<RoleResponse>> {
        self.get_user_roles(user_id).await
    }
}
