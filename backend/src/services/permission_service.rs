use crate::models::{Permission, PermissionResponse, PermissionTree};
use crate::services::casbin_service::CasbinService;
use crate::utils::ApiResult;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct PermissionService {
    pool: SqlitePool,
    casbin_service: Arc<CasbinService>,
}

impl PermissionService {
    pub fn new(pool: SqlitePool, casbin_service: Arc<CasbinService>) -> Self {
        Self { pool, casbin_service }
    }

    /// Get all permissions
    pub async fn list_permissions(&self) -> ApiResult<Vec<PermissionResponse>> {
        let permissions: Vec<Permission> =
            sqlx::query_as("SELECT * FROM permissions ORDER BY type, code")
                .fetch_all(&self.pool)
                .await?;

        Ok(permissions.into_iter().map(|p| p.into()).collect())
    }

    /// Get menu permissions only
    pub async fn list_menu_permissions(&self) -> ApiResult<Vec<PermissionResponse>> {
        let permissions: Vec<Permission> =
            sqlx::query_as("SELECT * FROM permissions WHERE type = 'menu' ORDER BY code")
                .fetch_all(&self.pool)
                .await?;

        Ok(permissions.into_iter().map(|p| p.into()).collect())
    }

    /// Get API permissions only
    pub async fn list_api_permissions(&self) -> ApiResult<Vec<PermissionResponse>> {
        let permissions: Vec<Permission> =
            sqlx::query_as("SELECT * FROM permissions WHERE type = 'api' ORDER BY code")
                .fetch_all(&self.pool)
                .await?;

        Ok(permissions.into_iter().map(|p| p.into()).collect())
    }

    /// Get permissions as tree structure
    pub async fn get_permission_tree(&self) -> ApiResult<Vec<PermissionTree>> {
        let permissions: Vec<Permission> =
            sqlx::query_as("SELECT * FROM permissions ORDER BY type, code")
                .fetch_all(&self.pool)
                .await?;

        // Build tree structure
        let mut tree_map = std::collections::HashMap::<Option<i64>, Vec<PermissionTree>>::new();

        // First pass: create all nodes
        for perm in permissions {
            let node = PermissionTree {
                id: perm.id,
                code: perm.code.clone(),
                name: perm.name.clone(),
                r#type: perm.r#type.clone(),
                resource: perm.resource.clone(),
                action: perm.action.clone(),
                description: perm.description.clone(),
                children: vec![],
            };

            tree_map.entry(perm.parent_id).or_default().push(node);
        }

        // Second pass: build tree by connecting children
        fn build_tree(
            parent_id: Option<i64>,
            tree_map: &std::collections::HashMap<Option<i64>, Vec<PermissionTree>>,
        ) -> Vec<PermissionTree> {
            if let Some(children) = tree_map.get(&parent_id) {
                children
                    .iter()
                    .map(|node| PermissionTree {
                        id: node.id,
                        code: node.code.clone(),
                        name: node.name.clone(),
                        r#type: node.r#type.clone(),
                        resource: node.resource.clone(),
                        action: node.action.clone(),
                        description: node.description.clone(),
                        children: build_tree(Some(node.id), tree_map),
                    })
                    .collect()
            } else {
                vec![]
            }
        }

        Ok(build_tree(None, &tree_map))
    }

    /// Get user's all permissions (flat list)
    pub async fn get_user_permissions(&self, user_id: i64) -> ApiResult<Vec<PermissionResponse>> {
        let permissions: Vec<Permission> = sqlx::query_as(
            r#"
            SELECT DISTINCT p.*
            FROM permissions p
            JOIN role_permissions rp ON p.id = rp.permission_id
            JOIN user_roles ur ON rp.role_id = ur.role_id
            WHERE ur.user_id = ?
            ORDER BY p.type, p.code
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(permissions.into_iter().map(|p| p.into()).collect())
    }

    /// Check if user has permission
    pub async fn check_permission(
        &self,
        user_id: i64,
        resource: &str,
        action: &str,
    ) -> ApiResult<bool> {
        // Use Casbin to check permission
        self.casbin_service.enforce(user_id, resource, action).await
    }
}
