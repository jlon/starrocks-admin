use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bcrypt::{DEFAULT_COST, hash};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool, Transaction, sqlite::Sqlite};

use crate::models::{
    AdminCreateUserRequest, AdminUpdateUserRequest, RoleResponse, User, UserWithRolesResponse,
};
use crate::services::casbin_service::CasbinService;
use crate::utils::{ApiError, ApiResult};

#[derive(FromRow)]
struct UserRoleRecord {
    user_id: i64,
    id: i64,
    code: String,
    name: String,
    description: Option<String>,
    is_system: bool,
    created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct UserService {
    pool: SqlitePool,
    casbin_service: Arc<CasbinService>,
}

impl UserService {
    pub fn new(pool: SqlitePool, casbin_service: Arc<CasbinService>) -> Self {
        Self { pool, casbin_service }
    }

    pub async fn list_users(&self) -> ApiResult<Vec<UserWithRolesResponse>> {
        let users: Vec<User> = sqlx::query_as("SELECT * FROM users ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let roles_map = self.load_all_user_roles().await?;

        Ok(users
            .into_iter()
            .map(|user| {
                let roles = roles_map.get(&user.id);
                self.compose_user(user, roles)
            })
            .collect())
    }

    pub async fn get_user(&self, user_id: i64) -> ApiResult<UserWithRolesResponse> {
        let user = self.fetch_user(user_id).await?;
        let roles = self.fetch_user_roles(user_id).await?;
        Ok(UserWithRolesResponse { user: user.into(), roles })
    }

    pub async fn create_user(
        &self,
        req: AdminCreateUserRequest,
    ) -> ApiResult<UserWithRolesResponse> {
        let mut tx = self.pool.begin().await?;

        self.ensure_username_available(&mut tx, &req.username, None)
            .await?;

        let password_hash = hash(&req.password, DEFAULT_COST)
            .map_err(|err| ApiError::internal_error(format!("Failed to hash password: {}", err)))?;

        let result = {
            let conn = tx.as_mut();
            sqlx::query(
                "INSERT INTO users (username, password_hash, email, avatar) VALUES (?, ?, ?, ?)",
            )
            .bind(&req.username)
            .bind(&password_hash)
            .bind(&req.email)
            .bind(&req.avatar)
            .execute(conn)
            .await?
        };

        let user_id = result.last_insert_rowid();

        if let Some(role_ids) = &req.role_ids {
            self.replace_user_roles(&mut tx, user_id, role_ids).await?;
        }

        tx.commit().await?;

        self.get_user(user_id).await
    }

    pub async fn update_user(
        &self,
        user_id: i64,
        req: AdminUpdateUserRequest,
    ) -> ApiResult<UserWithRolesResponse> {
        let mut tx = self.pool.begin().await?;

        self.fetch_user_in_tx(&mut tx, user_id).await?;

        if let Some(username) = &req.username {
            self.ensure_username_available(&mut tx, username, Some(user_id))
                .await?;
            {
                let conn = tx.as_mut();
                sqlx::query(
                    "UPDATE users SET username = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(username)
                .bind(user_id)
                .execute(conn)
                .await?;
            }
        }

        if let Some(email) = &req.email {
            {
                let conn = tx.as_mut();
                sqlx::query(
                    "UPDATE users SET email = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(email)
                .bind(user_id)
                .execute(conn)
                .await?;
            }
        }

        if let Some(avatar) = &req.avatar {
            {
                let conn = tx.as_mut();
                sqlx::query(
                    "UPDATE users SET avatar = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(avatar)
                .bind(user_id)
                .execute(conn)
                .await?;
            }
        }

        if let Some(password) = &req.password {
            let password_hash = hash(password, DEFAULT_COST).map_err(|err| {
                ApiError::internal_error(format!("Failed to hash password: {}", err))
            })?;

            {
                let conn = tx.as_mut();
                sqlx::query(
                    "UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(&password_hash)
                .bind(user_id)
                .execute(conn)
                .await?;
            }
        }

        if let Some(role_ids) = &req.role_ids {
            self.replace_user_roles(&mut tx, user_id, role_ids).await?;
        }

        tx.commit().await?;

        self.get_user(user_id).await
    }

    pub async fn delete_user(&self, user_id: i64) -> ApiResult<()> {
        let mut tx = self.pool.begin().await?;

        self.fetch_user_in_tx(&mut tx, user_id).await?;
        let current_role_ids = self.collect_user_role_ids(&mut tx, user_id).await?;

        {
            let conn = tx.as_mut();
            sqlx::query("DELETE FROM user_roles WHERE user_id = ?")
                .bind(user_id)
                .execute(conn)
                .await?;
        }

        {
            let conn = tx.as_mut();
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(user_id)
                .execute(conn)
                .await?;
        }

        tx.commit().await?;

        for role_id in current_role_ids {
            let _ = self
                .casbin_service
                .remove_role_for_user(user_id, role_id)
                .await;
        }

        Ok(())
    }

    fn compose_user(&self, user: User, roles: Option<&Vec<RoleResponse>>) -> UserWithRolesResponse {
        UserWithRolesResponse { user: user.into(), roles: roles.cloned().unwrap_or_default() }
    }

    async fn fetch_user(&self, user_id: i64) -> ApiResult<User> {
        sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("User not found"))
    }

    async fn fetch_user_in_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        user_id: i64,
    ) -> ApiResult<User> {
        let conn = tx.as_mut();
        sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(conn)
            .await?
            .ok_or_else(|| ApiError::not_found("User not found"))
    }

    async fn ensure_username_available(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        username: &str,
        current_user: Option<i64>,
    ) -> ApiResult<()> {
        let existing: Option<(i64,)> = {
            let conn = tx.as_mut();
            sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(conn)
                .await?
        };

        if let Some((id,)) = existing {
            if current_user.map(|uid| uid != id).unwrap_or(true) {
                return Err(ApiError::validation_error("Username already exists"));
            }
        }

        Ok(())
    }

    async fn replace_user_roles(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        user_id: i64,
        role_ids: &[i64],
    ) -> ApiResult<()> {
        let unique_ids: HashSet<i64> = role_ids.iter().copied().collect();
        self.validate_roles(tx, &unique_ids).await?;

        let current_ids = self.collect_user_role_ids(tx, user_id).await?;
        let current_set: HashSet<i64> = current_ids.iter().copied().collect();

        let to_add: Vec<i64> = unique_ids.difference(&current_set).copied().collect();
        let to_remove: Vec<i64> = current_set.difference(&unique_ids).copied().collect();

        for role_id in &to_remove {
            {
                let conn = tx.as_mut();
                sqlx::query("DELETE FROM user_roles WHERE user_id = ? AND role_id = ?")
                    .bind(user_id)
                    .bind(role_id)
                    .execute(conn)
                    .await?;
            }

            self.casbin_service
                .remove_role_for_user(user_id, *role_id)
                .await?;
        }

        for role_id in &to_add {
            {
                let conn = tx.as_mut();
                sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(role_id)
                    .execute(conn)
                    .await?;
            }

            self.casbin_service
                .add_role_for_user(user_id, *role_id)
                .await?;
        }

        Ok(())
    }

    async fn validate_roles(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        role_ids: &HashSet<i64>,
    ) -> ApiResult<()> {
        if role_ids.is_empty() {
            return Ok(());
        }

        for role_id in role_ids {
            let exists: Option<(i64,)> = {
                let conn = tx.as_mut();
                sqlx::query_as("SELECT id FROM roles WHERE id = ?")
                    .bind(role_id)
                    .fetch_optional(conn)
                    .await?
            };

            if exists.is_none() {
                return Err(ApiError::not_found(format!("Role {} not found", role_id)));
            }
        }

        Ok(())
    }

    async fn collect_user_role_ids(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        user_id: i64,
    ) -> ApiResult<Vec<i64>> {
        let rows: Vec<(i64,)> = {
            let conn = tx.as_mut();
            sqlx::query_as("SELECT role_id FROM user_roles WHERE user_id = ?")
                .bind(user_id)
                .fetch_all(conn)
                .await?
        };

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn fetch_user_roles(&self, user_id: i64) -> ApiResult<Vec<RoleResponse>> {
        let rows: Vec<UserRoleRecord> = sqlx::query_as(
            r#"
            SELECT ur.user_id, r.*
            FROM user_roles ur
            JOIN roles r ON r.id = ur.role_id
            WHERE ur.user_id = ?
            ORDER BY r.name
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| self.map_role(row)).collect())
    }

    async fn load_all_user_roles(&self) -> ApiResult<HashMap<i64, Vec<RoleResponse>>> {
        let rows: Vec<UserRoleRecord> = sqlx::query_as(
            r#"
            SELECT ur.user_id, r.*
            FROM user_roles ur
            JOIN roles r ON r.id = ur.role_id
            ORDER BY r.name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut map: HashMap<i64, Vec<RoleResponse>> = HashMap::new();
        for row in rows {
            map.entry(row.user_id).or_default().push(self.map_role(row));
        }
        Ok(map)
    }

    fn map_role(&self, row: UserRoleRecord) -> RoleResponse {
        RoleResponse {
            id: row.id,
            code: row.code,
            name: row.name,
            description: row.description,
            is_system: row.is_system,
            created_at: row.created_at,
        }
    }
}
