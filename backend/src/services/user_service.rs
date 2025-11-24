use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bcrypt::{DEFAULT_COST, hash};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool, Transaction, sqlite::Sqlite};

use crate::models::{
    AdminCreateUserRequest, AdminUpdateUserRequest, RoleResponse, User, UserWithRolesResponse,
};
use crate::services::casbin_service::CasbinService;
use crate::utils::organization_filter::apply_organization_filter;
use crate::utils::{ApiError, ApiResult};

#[derive(FromRow)]
struct UserRoleRecord {
    user_id: i64,
    id: i64,
    code: String,
    name: String,
    description: Option<String>,
    is_system: bool,
    organization_id: Option<i64>,
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

    pub async fn list_users(
        &self,
        organization_id: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<Vec<UserWithRolesResponse>> {
        let base_query = "SELECT u.*, o.name as organization_name FROM users u LEFT JOIN organizations o ON u.organization_id = o.id ORDER BY u.created_at DESC";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, organization_id);

        // Query with organization name
        #[derive(FromRow)]
        struct UserWithOrgName {
            id: i64,
            username: String,
            password_hash: String,
            email: Option<String>,
            avatar: Option<String>,
            organization_id: Option<i64>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            organization_name: Option<String>,
        }

        let users_with_org: Vec<UserWithOrgName> = sqlx::query_as(&filtered_query)
            .fetch_all(&self.pool)
            .await?;

        let roles_map = self.load_all_user_roles().await?;

        Ok(users_with_org
            .into_iter()
            .map(|user_with_org| {
                let user = User {
                    id: user_with_org.id,
                    username: user_with_org.username,
                    password_hash: user_with_org.password_hash,
                    email: user_with_org.email,
                    avatar: user_with_org.avatar,
                    organization_id: user_with_org.organization_id,
                    created_at: user_with_org.created_at,
                    updated_at: user_with_org.updated_at,
                };
                let roles = roles_map.get(&user.id);
                self.compose_user_with_org(user, user_with_org.organization_name, roles)
            })
            .collect())
    }

    pub async fn get_user(
        &self,
        user_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<UserWithRolesResponse> {
        let user = self
            .fetch_user(user_id, requestor_org, is_super_admin)
            .await?;
        let roles = self.fetch_user_roles(user_id).await?;
        Ok(UserWithRolesResponse { user: user.into(), roles })
    }

    pub async fn create_user(
        &self,
        req: AdminCreateUserRequest,
        organization_id: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<UserWithRolesResponse> {
        // Enforce organization context for non-super admin
        if !is_super_admin && organization_id.is_none() {
            return Err(ApiError::forbidden("Organization context required for user creation"));
        }

        let target_org_id = self
            .resolve_target_org(req.organization_id, organization_id, is_super_admin)
            .await?;

        let mut tx = self.pool.begin().await?;

        self.ensure_username_available(&mut tx, &req.username, None)
            .await?;

        let password_hash = hash(&req.password, DEFAULT_COST)
            .map_err(|err| ApiError::internal_error(format!("Failed to hash password: {}", err)))?;

        let result = {
            let conn = tx.as_mut();
            sqlx::query(
                "INSERT INTO users (username, password_hash, email, avatar, organization_id) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&req.username)
            .bind(&password_hash)
            .bind(&req.email)
            .bind(&req.avatar)
            .bind(target_org_id)
            .execute(conn)
            .await?
        };

        let user_id = result.last_insert_rowid();

        // Assign user to organization
        self.upsert_user_organization(&mut tx, user_id, target_org_id)
            .await?;

        if let Some(role_ids) = &req.role_ids {
            self.replace_user_roles(
                &mut tx,
                user_id,
                role_ids,
                Some(target_org_id),
                is_super_admin,
            )
            .await?;
        }

        tx.commit().await?;

        self.get_user(user_id, organization_id, is_super_admin)
            .await
    }

    pub async fn update_user(
        &self,
        user_id: i64,
        req: AdminUpdateUserRequest,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<UserWithRolesResponse> {
        let mut tx = self.pool.begin().await?;

        let existing_user = self
            .fetch_user_in_tx(&mut tx, user_id, requestor_org, is_super_admin)
            .await?;

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
            self.replace_user_roles(&mut tx, user_id, role_ids, requestor_org, is_super_admin)
                .await?;
        }

        if let Some(new_org_id) = req.organization_id {
            // Only check if organization_id is being changed (not just present)
            if !is_super_admin && Some(new_org_id) != existing_user.organization_id {
                return Err(ApiError::forbidden(
                    "Only super administrators can reassign user organizations",
                ));
            }
            self.ensure_organization_exists(new_org_id).await?;
            {
                let conn = tx.as_mut();
                sqlx::query(
                    "UPDATE users SET organization_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(new_org_id)
                .bind(user_id)
                .execute(conn)
                .await?;
            }

            self.upsert_user_organization(&mut tx, user_id, new_org_id)
                .await?;
        }

        tx.commit().await?;

        self.get_user(user_id, requestor_org, is_super_admin).await
    }

    async fn upsert_user_organization(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        user_id: i64,
        org_id: i64,
    ) -> ApiResult<()> {
        let conn = tx.as_mut();
        sqlx::query(
            r#"
            INSERT INTO user_organizations (user_id, organization_id)
            VALUES (?, ?)
            ON CONFLICT(user_id) DO UPDATE SET organization_id = excluded.organization_id
            "#,
        )
        .bind(user_id)
        .bind(org_id)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete_user(
        &self,
        user_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<()> {
        let mut tx = self.pool.begin().await?;

        self.fetch_user_in_tx(&mut tx, user_id, requestor_org, is_super_admin)
            .await?;
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

    fn compose_user_with_org(
        &self,
        user: User,
        organization_name: Option<String>,
        roles: Option<&Vec<RoleResponse>>,
    ) -> UserWithRolesResponse {
        use crate::models::UserResponse;

        // Check if user is organization admin by checking roles
        let is_org_admin =
            roles.is_some_and(|r| r.iter().any(|role| role.code.starts_with("org_admin_")));

        let user_response =
            UserResponse::from_user_with_org(user, organization_name, false, is_org_admin);
        UserWithRolesResponse { user: user_response, roles: roles.cloned().unwrap_or_default() }
    }

    async fn fetch_user(
        &self,
        user_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<User> {
        let base_query = "SELECT * FROM users WHERE id = ?";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, requestor_org);
        sqlx::query_as(&filtered_query)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("User not found"))
    }

    async fn fetch_user_in_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        user_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<User> {
        let base_query = "SELECT * FROM users WHERE id = ?";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, requestor_org);
        let conn = tx.as_mut();
        sqlx::query_as(&filtered_query)
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

        if let Some((id,)) = existing
            && current_user.map(|uid| uid != id).unwrap_or(true)
        {
            return Err(ApiError::validation_error("Username already exists"));
        }

        Ok(())
    }

    async fn replace_user_roles(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        user_id: i64,
        role_ids: &[i64],
        organization_id: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<()> {
        let unique_ids: HashSet<i64> = role_ids.iter().copied().collect();
        self.validate_roles(tx, &unique_ids, organization_id, is_super_admin)
            .await?;

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
        organization_id: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<()> {
        if role_ids.is_empty() {
            return Ok(());
        }

        for role_id in role_ids {
            let base_query = "SELECT id FROM roles WHERE id = ?";
            let (filtered_query, _) =
                apply_organization_filter(base_query, is_super_admin, organization_id);
            let exists: Option<(i64,)> = {
                let conn = tx.as_mut();
                sqlx::query_as(&filtered_query)
                    .bind(role_id)
                    .fetch_optional(conn)
                    .await?
            };

            if exists.is_none() {
                return Err(ApiError::not_found(format!(
                    "Role {} not found or not accessible in this organization",
                    role_id
                )));
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
            organization_id: row.organization_id,
            created_at: row.created_at,
        }
    }

    async fn resolve_target_org(
        &self,
        requested_org: Option<i64>,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<i64> {
        if is_super_admin {
            if let Some(id) = requested_org.or(requestor_org) {
                self.ensure_organization_exists(id).await?;
                return Ok(id);
            }
            return self.fetch_default_org_id().await;
        }

        // For non-super admins, ensure they can only create users in their own organization
        let current_org = requestor_org.ok_or_else(|| {
            ApiError::forbidden("Organization context required for user creation")
        })?;

        // If organization_id is explicitly specified, it must match the requestor's organization
        if let Some(requested_id) = requested_org
            && requested_id != current_org
        {
            return Err(ApiError::forbidden(
                "Organization administrators cannot create users in other organizations",
            ));
        }

        Ok(current_org)
    }

    async fn ensure_organization_exists(&self, org_id: i64) -> ApiResult<()> {
        let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM organizations WHERE id = ?")
            .bind(org_id)
            .fetch_optional(&self.pool)
            .await?;
        if exists.is_none() {
            return Err(ApiError::not_found("Organization not found"));
        }
        Ok(())
    }

    async fn fetch_default_org_id(&self) -> ApiResult<i64> {
        if let Some(id) =
            sqlx::query_scalar("SELECT id FROM organizations WHERE code = 'default_org'")
                .fetch_optional(&self.pool)
                .await?
        {
            return Ok(id);
        }

        sqlx::query("INSERT INTO organizations (code, name, description, is_system) VALUES ('default_org', 'Default Organization', 'Auto-created default organization', 1)")
            .execute(&self.pool)
            .await?;

        sqlx::query_scalar("SELECT id FROM organizations WHERE code = 'default_org'")
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Default organization not found"))
    }
}
