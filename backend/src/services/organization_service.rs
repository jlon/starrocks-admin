use crate::models::{
    CreateOrganizationRequest, Organization, OrganizationResponse, UpdateOrganizationRequest,
};
use crate::utils::{ApiError, ApiResult};
use bcrypt::{DEFAULT_COST, hash};
use sqlx::{SqlitePool, Transaction};

#[derive(Clone)]
pub struct OrganizationService {
    pool: SqlitePool,
}

impl OrganizationService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // Create organization and optionally create admin user (Option A)
    pub async fn create_organization(
        &self,
        req: CreateOrganizationRequest,
    ) -> ApiResult<OrganizationResponse> {
        let admin_plan = Self::resolve_admin_plan(&req)?;
        let mut tx = self.pool.begin().await?;

        // Ensure code unique
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM organizations WHERE code = ?")
                .bind(&req.code)
                .fetch_optional(&mut *tx)
                .await?;
        if existing.is_some() {
            return Err(ApiError::validation_error("Organization code already exists"));
        }

        // Insert organization
        let result = sqlx::query(
            "INSERT INTO organizations (code, name, description, is_system) VALUES (?, ?, ?, ?)",
        )
        .bind(&req.code)
        .bind(&req.name)
        .bind(&req.description)
        .bind(false) // not system
        .execute(&mut *tx)
        .await?;
        let org_id = result.last_insert_rowid();

        // Create org_admin role scoped to this organization
        let role_id = self
            .create_org_admin_role(&mut tx, org_id, &req.code, &req.name)
            .await?;

        // Provision administrator when requested
        if let Some(plan) = admin_plan {
            let admin_user_id = match plan {
                AdminPlan::Create { username, password, email } => {
                    let password_hash = hash(&password, DEFAULT_COST).map_err(|e| {
                        ApiError::internal_error(format!("Failed to hash admin password: {}", e))
                    })?;
                    self.create_admin_user(&mut tx, &username, &password_hash, email, org_id)
                        .await?
                },
                AdminPlan::Existing(user_id) => {
                    self.assign_existing_admin(&mut tx, user_id, org_id).await?;
                    user_id
                },
            };

            self.assign_role_to_user(&mut tx, admin_user_id, role_id)
                .await?;
        }

        tx.commit().await?;

        let org: Organization = sqlx::query_as("SELECT * FROM organizations WHERE id = ?")
            .bind(org_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(org.into())
    }

    // List organizations (super admin sees all, others see only their own)
    pub async fn list_organizations(
        &self,
        org_id: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<Vec<OrganizationResponse>> {
        let orgs: Vec<Organization> = if is_super_admin {
            sqlx::query_as("SELECT * FROM organizations ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?
        } else if let Some(org) = org_id {
            sqlx::query_as("SELECT * FROM organizations WHERE id = ? ORDER BY created_at DESC")
                .bind(org)
                .fetch_all(&self.pool)
                .await?
        } else {
            vec![]
        };

        Ok(orgs.into_iter().map(|o| o.into()).collect())
    }

    pub async fn get_organization(
        &self,
        id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<OrganizationResponse> {
        let org: Option<Organization> = sqlx::query_as("SELECT * FROM organizations WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        let org = org.ok_or_else(|| ApiError::not_found("Organization not found"))?;

        // Enforce org scope unless super admin
        if !is_super_admin && Some(org.id) != requestor_org {
            return Err(ApiError::forbidden("Access to this organization is not allowed"));
        }

        // Get admin user ID for this organization
        let admin_user_id = self.get_org_admin_user_id(id).await?;

        Ok(OrganizationResponse::from(org).with_admin(admin_user_id))
    }

    async fn get_org_admin_user_id(&self, org_id: i64) -> ApiResult<Option<i64>> {
        let admin_user_id: Option<(i64,)> = sqlx::query_as(
            "SELECT ur.user_id 
             FROM user_roles ur
             JOIN roles r ON ur.role_id = r.id
             WHERE r.organization_id = ? AND r.code LIKE 'org_admin_%'
             LIMIT 1",
        )
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(admin_user_id.map(|(id,)| id))
    }

    pub async fn update_organization(
        &self,
        id: i64,
        req: UpdateOrganizationRequest,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<OrganizationResponse> {
        // Verify existence and permissions
        let _existing = self
            .get_organization(id, requestor_org, is_super_admin)
            .await?;

        let mut tx = self.pool.begin().await?;

        // Build dynamic update
        let mut updates = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(name) = &req.name {
            updates.push("name = ?");
            params.push(name.clone());
        }
        if let Some(desc) = &req.description {
            updates.push("description = ?");
            params.push(desc.clone());
        }

        if !updates.is_empty() {
            updates.push("updated_at = CURRENT_TIMESTAMP");
            let sql = format!("UPDATE organizations SET {} WHERE id = ?", updates.join(", "));

            let mut query = sqlx::query(&sql);
            for p in params {
                query = query.bind(p);
            }
            query = query.bind(id);

            query.execute(&mut *tx).await?;
        }

        if let Some(admin_user_id) = req.admin_user_id {
            self.assign_org_admin_user(&mut tx, id, admin_user_id)
                .await?;
        }

        tx.commit().await?;

        self.get_organization(id, requestor_org, is_super_admin)
            .await
    }

    pub async fn delete_organization(
        &self,
        id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<()> {
        // Verify existence and permissions
        let org = self
            .get_organization(id, requestor_org, is_super_admin)
            .await?;

        // Prevent deletion of system organizations (e.g., default_org)
        if org.is_system {
            return Err(ApiError::forbidden("System organizations cannot be deleted"));
        }

        self.ensure_organization_empty(org.id).await?;

        // Delete organization (cascades to user_organizations)
        let result = sqlx::query("DELETE FROM organizations WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(ApiError::not_found("Organization not found"));
        }

        Ok(())
    }

    // Helper: get user organization
    pub async fn get_user_organization(&self, user_id: i64) -> ApiResult<Option<i64>> {
        let org_id: Option<i64> =
            sqlx::query_scalar("SELECT organization_id FROM user_organizations WHERE user_id = ?")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(org_id)
    }

    fn resolve_admin_plan(req: &CreateOrganizationRequest) -> ApiResult<Option<AdminPlan>> {
        match (req.admin_user_id, req.admin_username.as_ref(), req.admin_password.as_ref()) {
            (Some(existing_id), None, None) => Ok(Some(AdminPlan::Existing(existing_id))),
            (None, Some(username), Some(password)) => Ok(Some(AdminPlan::Create {
                username: username.clone(),
                password: password.clone(),
                email: req.admin_email.clone(),
            })),
            (None, None, None) => Ok(None),
            _ => Err(ApiError::validation_error(
                "Provide either admin_user_id or admin_username/admin_password",
            )),
        }
    }

    async fn create_org_admin_role(
        &self,
        tx: &mut Transaction<'_, sqlx::Sqlite>,
        org_id: i64,
        org_code: &str,
        org_name: &str,
    ) -> ApiResult<i64> {
        let role_result = sqlx::query(
            "INSERT INTO roles (code, name, description, is_system, organization_id) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(format!("org_admin_{}", org_code))
        .bind(format!("Organization Admin ({})", org_name))
        .bind(format!("Admin for organization {}", org_name))
        .bind(false)
        .bind(org_id)
        .execute(&mut **tx)
        .await?;
        let role_id = role_result.last_insert_rowid();

        // Grant all permissions except organization management
        // Exclude:
        // 1. menu:system:organizations - Organization management menu
        // 2. api:organizations:* - All organization management APIs
        // Exclude:
        // 1. menu:system:organizations - Organization management menu
        // 2. api:organizations:* - All organization management APIs
        let perms = sqlx::query_as::<_, (i64,)>(
            "SELECT id FROM permissions 
             WHERE code NOT IN ('menu:system:organizations')
               AND code NOT LIKE 'api:organizations:%'",
        )
        .fetch_all(&mut **tx)
        .await?;

        let perm_count = perms.len();
        for (pid,) in perms {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)")
                .bind(role_id)
                .bind(pid)
                .execute(&mut **tx)
                .await?;
        }

        tracing::info!(
            "Created org_admin role for organization {} with {} permissions (excluding organization management)",
            org_code,
            perm_count
        );

        Ok(role_id)
    }

    async fn create_admin_user(
        &self,
        tx: &mut Transaction<'_, sqlx::Sqlite>,
        username: &str,
        password_hash: &str,
        email: Option<String>,
        org_id: i64,
    ) -> ApiResult<i64> {
        let user_result = sqlx::query(
            "INSERT INTO users (username, password_hash, email, organization_id) VALUES (?, ?, ?, ?)",
        )
        .bind(username)
        .bind(password_hash)
        .bind(email)
        .bind(org_id)
        .execute(&mut **tx)
        .await?;
        let user_id = user_result.last_insert_rowid();

        self.upsert_user_organization(tx, user_id, org_id).await?;
        Ok(user_id)
    }

    async fn assign_existing_admin(
        &self,
        tx: &mut Transaction<'_, sqlx::Sqlite>,
        user_id: i64,
        org_id: i64,
    ) -> ApiResult<()> {
        let exists = sqlx::query_scalar::<_, Option<i64>>("SELECT id FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&mut **tx)
            .await?;
        if exists.is_none() {
            return Err(ApiError::not_found("Admin user not found"));
        }

        sqlx::query("UPDATE users SET organization_id = ? WHERE id = ?")
            .bind(org_id)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;

        self.upsert_user_organization(tx, user_id, org_id).await
    }

    async fn upsert_user_organization(
        &self,
        tx: &mut Transaction<'_, sqlx::Sqlite>,
        user_id: i64,
        org_id: i64,
    ) -> ApiResult<()> {
        sqlx::query(
            r#"
            INSERT INTO user_organizations (user_id, organization_id)
            VALUES (?, ?)
            ON CONFLICT(user_id) DO UPDATE SET organization_id = excluded.organization_id
            "#,
        )
        .bind(user_id)
        .bind(org_id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn assign_org_admin_user(
        &self,
        tx: &mut Transaction<'_, sqlx::Sqlite>,
        org_id: i64,
        user_id: i64,
    ) -> ApiResult<()> {
        let user_org: Option<i64> =
            sqlx::query_scalar("SELECT organization_id FROM user_organizations WHERE user_id = ?")
                .bind(user_id)
                .fetch_optional(&mut **tx)
                .await?;

        match user_org {
            Some(existing_org) if existing_org == org_id => {},
            Some(_) => {
                return Err(ApiError::validation_error(
                    "Selected user must belong to this organization",
                ));
            },
            None => {
                return Err(ApiError::validation_error(
                    "Selected user is not assigned to any organization",
                ));
            },
        }

        let role_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM roles WHERE organization_id = ? AND code LIKE 'org_admin_%' LIMIT 1",
        )
        .bind(org_id)
        .fetch_optional(&mut **tx)
        .await?;

        let role_id = role_id.ok_or_else(|| {
            ApiError::internal_error("Organization admin role is missing for this organization")
        })?;

        sqlx::query("DELETE FROM user_roles WHERE role_id = ?")
            .bind(role_id)
            .execute(&mut **tx)
            .await?;

        sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(role_id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    async fn assign_role_to_user(
        &self,
        tx: &mut Transaction<'_, sqlx::Sqlite>,
        user_id: i64,
        role_id: i64,
    ) -> ApiResult<()> {
        sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(role_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn ensure_organization_empty(&self, org_id: i64) -> ApiResult<()> {
        let user_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_organizations WHERE organization_id = ?")
                .bind(org_id)
                .fetch_one(&self.pool)
                .await?;
        if user_count > 0 {
            return Err(ApiError::validation_error(
                "Organization still has users, please migrate them before deletion",
            ));
        }

        let cluster_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM clusters WHERE organization_id = ?")
                .bind(org_id)
                .fetch_one(&self.pool)
                .await?;
        if cluster_count > 0 {
            return Err(ApiError::validation_error(
                "Organization still has clusters, please delete them before deletion",
            ));
        }

        let role_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM roles WHERE organization_id = ?")
                .bind(org_id)
                .fetch_one(&self.pool)
                .await?;
        if role_count > 0 {
            return Err(ApiError::validation_error(
                "Organization still has roles, please remove them before deletion",
            ));
        }

        Ok(())
    }
}

enum AdminPlan {
    Existing(i64),
    Create { username: String, password: String, email: Option<String> },
}
