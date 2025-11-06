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

#[derive(Clone)]
pub struct AuthState {
    pub jwt_util: Arc<JwtUtil>,
    pub casbin_service: Arc<CasbinService>,
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

    req.extensions_mut().insert(user_id);
    req.extensions_mut().insert(claims.username.clone());

    if let Some((resource, action)) = permission_extractor::extract_permission(&method, &uri) {
        tracing::debug!("Checking permission for user {} -> {}:{}", user_id, resource, action);

        let allowed = state
            .casbin_service
            .enforce(user_id, &resource, &action)
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
