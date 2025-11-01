use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

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
    let uri = req.uri().to_string();
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

    if let Some((resource, action)) = extract_permission_internal(&method, &uri) {
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

/// 测试辅助
#[cfg(test)]
pub fn extract_permission(method: &str, uri: &str) -> Option<(String, String)> {
    extract_permission_internal(method, uri)
}

fn extract_permission_internal(method: &str, uri: &str) -> Option<(String, String)> {
    if uri == "/api/auth/permissions" {
        return None;
    }

    let path = uri.strip_prefix("/api/").unwrap_or(uri);
    let segments: Vec<&str> = path.split('/').collect();

    let resource = match segments.first()? {
        &"roles" => "roles",
        &"permissions" => "permissions",
        &"users" => "users",
        &"clusters" => "clusters",
        _ => return None,
    };

    let action = match (resource, segments.len(), method) {
        ("roles", len, _) if len >= 3 && segments.get(2) == Some(&"permissions") => match method {
            "PUT" => Some("permissions:update"),
            "GET" => Some("permissions:get"),
            _ => None,
        },
        ("users", len, _) if len >= 3 && segments.get(2) == Some(&"roles") => match method {
            "POST" => Some("roles:assign"),
            "DELETE" => Some("roles:remove"),
            "GET" => Some("roles:get"),
            _ => None,
        },
        ("clusters", len, verb) if len >= 3 => {
            if let Some(last) = segments.last() {
                let leaf = *last;
                if leaf.parse::<i64>().is_err() {
                    Some(leaf)
                } else {
                    match verb {
                        "GET" => Some("get"),
                        "PUT" => Some("update"),
                        "DELETE" => Some("delete"),
                        _ => None,
                    }
                }
            } else {
                None
            }
        },
        (_, len, verb) => {
            if len >= 2 {
                let second = segments.get(1).copied();
                if second.and_then(|val| val.parse::<i64>().ok()).is_some() {
                    match verb {
                        "GET" => Some("get"),
                        "PUT" => Some("update"),
                        "DELETE" => Some("delete"),
                        _ => None,
                    }
                } else {
                    match verb {
                        "GET" => second,
                        _ => None,
                    }
                }
            } else {
                match verb {
                    "GET" => Some("list"),
                    "POST" => Some("create"),
                    _ => None,
                }
            }
        },
    }?;

    Some((resource.to_string(), action.to_string()))
}
