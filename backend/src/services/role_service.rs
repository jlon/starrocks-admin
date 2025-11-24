use crate::models::{
    CreateRoleRequest, PermissionResponse, Role, RoleResponse, RoleWithPermissions,
    UpdateRolePermissionsRequest, UpdateRoleRequest,
};
use crate::services::{casbin_service::CasbinService, permission_service::PermissionService};
use crate::utils::organization_filter::apply_organization_filter;
use crate::utils::{ApiError, ApiResult};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct RoleService {
    pool: SqlitePool,
    casbin_service: Arc<CasbinService>,
}

impl RoleService {
    pub fn new(
        pool: SqlitePool,
        casbin_service: Arc<CasbinService>,
        _permission_service: Arc<PermissionService>,
    ) -> Self {
        Self { pool, casbin_service }
    }

    /// List all roles (organization-scoped for non-super-admin)
    pub async fn list_roles(
        &self,
        organization_id: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<Vec<RoleResponse>> {
        let base_query = "SELECT * FROM roles ORDER BY is_system DESC, name";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, organization_id);
        let roles: Vec<Role> = sqlx::query_as(&filtered_query)
            .fetch_all(&self.pool)
            .await?;
        Ok(roles.into_iter().map(|r| r.into()).collect())
    }

    /// Get role by ID (organization-scoped)
    pub async fn get_role(
        &self,
        role_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<RoleResponse> {
        let base_query = "SELECT * FROM roles WHERE id = ?";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, requestor_org);
        let role: Role = sqlx::query_as(&filtered_query)
            .bind(role_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Role not found"))?;
        Ok(role.into())
    }

    /// Get role with permissions (organization-scoped)
    pub async fn get_role_with_permissions(
        &self,
        role_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<RoleWithPermissions> {
        let role = self
            .get_role(role_id, requestor_org, is_super_admin)
            .await?;

        let permissions: Vec<PermissionResponse> = sqlx::query_as(
            r#"
            SELECT p.*
            FROM permissions p
            JOIN role_permissions rp ON p.id = rp.permission_id
            WHERE rp.role_id = ?
            ORDER BY p.type, p.code
            "#,
        )
        .bind(role_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|p: crate::models::Permission| p.into())
        .collect();

        Ok(RoleWithPermissions { role, permissions })
    }

    /// Create a new role (organization-scoped)
    pub async fn create_role(
        &self,
        req: CreateRoleRequest,
        organization_id: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<RoleResponse> {
        // Enforce organization scope for role creation
        if !is_super_admin && organization_id.is_none() {
            return Err(ApiError::forbidden("Organization context required for role creation"));
        }

        let target_org =
            if is_super_admin { req.organization_id.or(organization_id) } else { organization_id };

        // Check if role code already exists
        let base_query = "SELECT * FROM roles WHERE code = ?";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, organization_id);
        let existing: Option<Role> = sqlx::query_as(&filtered_query)
            .bind(&req.code)
            .fetch_optional(&self.pool)
            .await?;

        if existing.is_some() {
            return Err(ApiError::validation_error(
                "Role code already exists in this organization",
            ));
        }

        // Insert new role
        let result = if is_super_admin && target_org.is_none() {
            // Super admin can create system-wide roles
            sqlx::query(
                "INSERT INTO roles (code, name, description, is_system) VALUES (?, ?, ?, 0)",
            )
            .bind(&req.code)
            .bind(&req.name)
            .bind(&req.description)
            .execute(&self.pool)
            .await?
        } else {
            sqlx::query(
                "INSERT INTO roles (code, name, description, is_system, organization_id) VALUES (?, ?, ?, 0, ?)",
            )
            .bind(&req.code)
            .bind(&req.name)
            .bind(&req.description)
            .bind(target_org)
            .execute(&self.pool)
            .await?
        };

        let role_id = result.last_insert_rowid();

        let role: Role = sqlx::query_as("SELECT * FROM roles WHERE id = ?")
            .bind(role_id)
            .fetch_one(&self.pool)
            .await?;

        tracing::info!(
            "Role created: {} (ID: {}) in org {:?}",
            role.name,
            role.id,
            role.organization_id
        );

        Ok(role.into())
    }

    /// Update role (organization-scoped)
    pub async fn update_role(
        &self,
        role_id: i64,
        req: UpdateRoleRequest,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<RoleResponse> {
        // Check if role exists and within org scope
        let base_query = "SELECT * FROM roles WHERE id = ?";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, requestor_org);
        let role: Role = sqlx::query_as(&filtered_query)
            .bind(role_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Role not found"))?;

        // System roles cannot be modified (except description)
        if role.is_system {
            // Only allow description update for system roles
            if req.name.is_some() {
                return Err(ApiError::validation_error("Cannot modify system role name"));
            }
        }

        // Build update query
        let mut update_parts = Vec::new();
        let mut bind_values: Vec<Box<dyn sqlx::Encode<'_, sqlx::Sqlite> + Send>> = Vec::new();

        if let Some(name) = &req.name {
            update_parts.push("name = ?");
            bind_values.push(Box::new(name.clone()));
        }

        if let Some(description) = &req.description {
            update_parts.push("description = ?");
            bind_values.push(Box::new(description.clone()));
        }

        if update_parts.is_empty() {
            return self.get_role(role_id, requestor_org, is_super_admin).await;
        }

        // Direct update approach
        if let Some(name) = req.name {
            if let Some(description) = req.description {
                sqlx::query("UPDATE roles SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(&name)
                    .bind(&description)
                    .bind(role_id)
                    .execute(&self.pool)
                    .await?;
            } else {
                sqlx::query(
                    "UPDATE roles SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(&name)
                .bind(role_id)
                .execute(&self.pool)
                .await?;
            }
        } else if let Some(description) = req.description {
            sqlx::query(
                "UPDATE roles SET description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&description)
            .bind(role_id)
            .execute(&self.pool)
            .await?;
        }

        if let Some(new_org_id) = req.organization_id {
            if !is_super_admin {
                return Err(ApiError::forbidden(
                    "Only super administrators can reassign role organization",
                ));
            }
            sqlx::query(
                "UPDATE roles SET organization_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(new_org_id)
            .bind(role_id)
            .execute(&self.pool)
            .await?;
        }

        self.get_role(role_id, requestor_org, is_super_admin).await
    }

    /// Delete role (organization-scoped)
    pub async fn delete_role(
        &self,
        role_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<()> {
        // Check if role exists and within org scope
        let base_query = "SELECT * FROM roles WHERE id = ?";
        let (filtered_query, _) =
            apply_organization_filter(base_query, is_super_admin, requestor_org);
        let role: Role = sqlx::query_as(&filtered_query)
            .bind(role_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Role not found"))?;

        if role.is_system {
            return Err(ApiError::validation_error("Cannot delete system role"));
        }

        // Delete role (cascade will handle role_permissions and user_roles)
        sqlx::query("DELETE FROM roles WHERE id = ?")
            .bind(role_id)
            .execute(&self.pool)
            .await?;

        // Reload Casbin policies
        self.casbin_service
            .reload_policies_from_db(&self.pool)
            .await?;

        tracing::info!("Role deleted: {} (ID: {})", role.name, role.id);

        Ok(())
    }

    /// Assign permissions to role (organization-scoped)
    /// Automatically associates API permissions with menu permissions based on parent_id
    pub async fn assign_permissions_to_role(
        &self,
        role_id: i64,
        req: UpdateRolePermissionsRequest,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<()> {
        // Check if role exists and within org scope
        let _role = self
            .get_role(role_id, requestor_org, is_super_admin)
            .await?;

        // Get all permissions to build menu->API mapping
        let all_permissions: Vec<crate::models::Permission> =
            sqlx::query_as("SELECT * FROM permissions ORDER BY type, code")
                .fetch_all(&self.pool)
                .await?;

        // Build menu->API mapping based on parent_id
        // This ensures that when a menu permission is assigned, its associated API permissions are also assigned
        let mut menu_to_apis: HashMap<i64, Vec<i64>> = HashMap::new();

        for api_perm in all_permissions.iter().filter(|p| p.r#type == "api") {
            if let Some(parent_id) = api_perm.parent_id {
                menu_to_apis.entry(parent_id).or_default().push(api_perm.id);
            }
        }

        // Extend permission list: add associated API permissions for selected menu permissions
        let mut extended_permission_ids = req.permission_ids.clone();

        for permission_id in &req.permission_ids {
            // Check if this is a menu permission
            if let Some(perm) = all_permissions.iter().find(|p| p.id == *permission_id)
                && perm.r#type == "menu"
            {
                // Automatically add associated API permissions
                if let Some(api_ids) = menu_to_apis.get(permission_id) {
                    extended_permission_ids.extend(api_ids.iter());
                    tracing::debug!(
                        "Menu permission {} (code: {}) auto-associated with {} API permissions",
                        permission_id,
                        perm.code,
                        api_ids.len()
                    );
                }
            }
        }

        // Remove duplicates and sort for consistency
        use std::collections::HashSet;
        let mut final_permission_ids: Vec<i64> = extended_permission_ids
            .into_iter()
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect();
        final_permission_ids.sort();

        let added_count = final_permission_ids.len() - req.permission_ids.len();
        if added_count > 0 {
            tracing::info!(
                "Auto-associated {} API permissions with menu permissions for role ID: {}",
                added_count,
                role_id
            );
        }

        // Begin transaction
        let mut tx = self.pool.begin().await?;

        // Delete existing role permissions
        sqlx::query("DELETE FROM role_permissions WHERE role_id = ?")
            .bind(role_id)
            .execute(&mut *tx)
            .await?;

        // Insert new role permissions (including auto-associated API permissions)
        for permission_id in &final_permission_ids {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)")
                .bind(role_id)
                .bind(permission_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        // Reload Casbin policies
        self.casbin_service
            .reload_policies_from_db(&self.pool)
            .await?;

        tracing::info!(
            "Permissions updated for role ID: {} (total: {} permissions, {} auto-associated)",
            role_id,
            final_permission_ids.len(),
            added_count
        );

        Ok(())
    }

    /// Get role permissions (organization-scoped)
    pub async fn get_role_permissions(
        &self,
        role_id: i64,
        requestor_org: Option<i64>,
        is_super_admin: bool,
    ) -> ApiResult<Vec<PermissionResponse>> {
        // Verify role exists and within org scope
        let _role = self
            .get_role(role_id, requestor_org, is_super_admin)
            .await?;

        let permissions: Vec<crate::models::Permission> = sqlx::query_as(
            r#"
            SELECT p.*
            FROM permissions p
            JOIN role_permissions rp ON p.id = rp.permission_id
            WHERE rp.role_id = ?
            ORDER BY p.type, p.code
            "#,
        )
        .bind(role_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(permissions.into_iter().map(|p| p.into()).collect())
    }
}
