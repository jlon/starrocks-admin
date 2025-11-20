use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::middleware::permission_extractor;
use crate::services::casbin_service::CasbinService;
use crate::utils::{ApiError, JwtUtil};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AuthState {
    pub jwt_util: Arc<JwtUtil>,
    pub casbin_service: Arc<CasbinService>,
    pub db: SqlitePool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct OrgContext {
    pub user_id: i64,
    #[allow(dead_code)]
    pub username: String,
    pub organization_id: Option<i64>,
    pub is_super_admin: bool,
}

/// Authentication + authorization middleware.
/// 1. 验证 JWT
/// 2. 将 `user_id` 写入 request extensions
/// 3. 根据 URI/Method 推导权限码并交给 Casbin 检查
pub async fn auth_middleware(
    State(state): State<AuthState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Extract path without query parameters
    let uri_full = req.uri().to_string();
    let uri = uri_full.split('?').next().unwrap_or(&uri_full).to_string();
    let method = req.method().to_string();

    tracing::debug!("Auth middleware processing: {} {}", method, uri);

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing authorization header for {} {}", method, uri);
            ApiError::unauthorized("Missing authorization header")
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        tracing::warn!("Invalid authorization header format for {} {}", method, uri);
        ApiError::unauthorized("Invalid authorization header format")
    })?;

    let claims = state.jwt_util.verify_token(token).map_err(|err| {
        tracing::warn!("JWT verification failed for {} {}: {:?}", method, uri, err);
        err
    })?;

    let user_id = claims.sub.parse::<i64>().unwrap_or_default();
    tracing::debug!(
        "JWT token verified for user {} (ID: {}) on {} {}",
        claims.username,
        user_id,
        method,
        uri
    );

    // Load organization and role info
    let is_super_admin: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
          SELECT 1
          FROM user_roles ur
          JOIN roles r ON r.id = ur.role_id
          WHERE ur.user_id = ? AND r.code = 'super_admin'
        )
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    // Prefer users.organization_id, fallback to user_organizations mapping
    let organization_id: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT organization_id FROM users WHERE id = ?
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .or_else(|| async_std_block_on_fetch_org(&state.db, user_id))
    .filter(|org| *org != 0);

    // Insert legacy extensions to keep backward compatibility
    req.extensions_mut().insert(user_id);
    req.extensions_mut().insert(claims.username.clone());

    // Insert org context for downstream services/handlers
    let org_ctx = OrgContext {
        user_id,
        username: claims.username.clone(),
        organization_id,
        is_super_admin,
    };
    req.extensions_mut().insert(org_ctx.clone());

    if let Some((resource, action)) = permission_extractor::extract_permission(&method, &uri) {
        let resource_scope = if org_ctx.is_super_admin || org_ctx.organization_id.is_none() {
            crate::services::casbin_service::CasbinService::format_resource_key(None, &resource)
        } else {
            crate::services::casbin_service::CasbinService::format_resource_key(
                org_ctx.organization_id,
                &resource,
            )
        };

        tracing::debug!(
            "Checking permission for user {} -> {}:{}",
            user_id,
            resource_scope,
            action
        );

        let allowed = state
            .casbin_service
            .enforce(user_id, &resource_scope, &action)
            .await
            .unwrap_or(false);

        if !allowed {
            tracing::warn!(
                "Permission denied for user {} on {} {} (resource={}, action={})",
                user_id,
                method,
                uri,
                resource,
                action
            );
            return Err(ApiError::unauthorized(format!(
                "Permission denied: no access to {} {}",
                resource, action
            )));
        }

        tracing::debug!("Permission granted for user {} on {} {}", user_id, method, uri);
    }

    Ok(next.run(req).await)
}

// Helper to fetch organization from user_organizations when users.organization_id is NULL
fn async_std_block_on_fetch_org(db: &SqlitePool, user_id: i64) -> Option<i64> {
    // Use a simple current-thread runtime to block on async safely in middleware context
    // Note: this is only a fallback path and executes a single short query.
    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        return handle.block_on(async move {
            sqlx::query_scalar::<_, i64>(
                r#"SELECT organization_id FROM user_organizations WHERE user_id = ?"#,
            )
            .bind(user_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
        });
    }
    None
}
