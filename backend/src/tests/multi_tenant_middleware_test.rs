// Multi-tenant middleware tests

use crate::middleware::{AuthState, OrgContext, auth_middleware};
use crate::tests::common::{
    create_test_casbin_service, create_test_db, setup_multi_tenant_test_data,
};
use crate::utils::JwtUtil;
use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode, header},
};
use std::sync::Arc;
use tower::ServiceExt;

/// Helper function to create test JWT token
fn create_test_token(jwt_util: &JwtUtil, user_id: i64, username: &str) -> String {
    jwt_util
        .generate_token(user_id, username)
        .expect("Failed to create test token")
}

/// Helper function to create test request with authentication
fn create_auth_request(token: &str, path: &str, method: Method) -> Request {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .expect("Failed to create test request")
}

/// Mock handler that extracts and returns OrgContext
async fn mock_handler(
    axum::extract::Extension(org_ctx): axum::extract::Extension<OrgContext>,
) -> axum::Json<OrgContext> {
    axum::Json(org_ctx)
}

#[tokio::test]
async fn test_super_admin_org_context() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create super admin token
    let token = create_test_token(&jwt_util, test_data.super_admin_user_id, "super_admin");

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make request
    let request = create_auth_request(&token, "/test", Method::GET);
    let response = app.oneshot(request).await.expect("Failed to make request");

    assert_eq!(response.status(), StatusCode::OK);

    // Extract and verify OrgContext
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let org_ctx: OrgContext =
        serde_json::from_slice(&body).expect("Failed to deserialize OrgContext");

    assert_eq!(org_ctx.user_id, test_data.super_admin_user_id);
    assert_eq!(org_ctx.username, "super_admin");
    assert_eq!(org_ctx.organization_id, None); // Super admin has no organization
    assert!(org_ctx.is_super_admin);
}

#[tokio::test]
async fn test_org_admin_org_context() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create org admin token
    let token = create_test_token(&jwt_util, test_data.org1_admin_user_id, "org1_admin");

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make request
    let request = create_auth_request(&token, "/test", Method::GET);
    let response = app.oneshot(request).await.expect("Failed to make request");

    assert_eq!(response.status(), StatusCode::OK);

    // Extract and verify OrgContext
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let org_ctx: OrgContext =
        serde_json::from_slice(&body).expect("Failed to deserialize OrgContext");

    assert_eq!(org_ctx.user_id, test_data.org1_admin_user_id);
    assert_eq!(org_ctx.username, "org1_admin");
    assert_eq!(org_ctx.organization_id, Some(test_data.org1_id));
    assert!(!org_ctx.is_super_admin);
}

#[tokio::test]
async fn test_regular_user_org_context() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create regular user token
    let token = create_test_token(&jwt_util, test_data.org1_regular_user_id, "org1_regular");

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make request
    let request = create_auth_request(&token, "/test", Method::GET);
    let response = app.oneshot(request).await.expect("Failed to make request");

    assert_eq!(response.status(), StatusCode::OK);

    // Extract and verify OrgContext
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let org_ctx: OrgContext =
        serde_json::from_slice(&body).expect("Failed to deserialize OrgContext");

    assert_eq!(org_ctx.user_id, test_data.org1_regular_user_id);
    assert_eq!(org_ctx.username, "org1_regular");
    assert_eq!(org_ctx.organization_id, Some(test_data.org1_id));
    assert!(!org_ctx.is_super_admin);
}

#[tokio::test]
async fn test_cross_organization_user_access() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create token for org1 user trying to access org2 resources
    let token = create_test_token(&jwt_util, test_data.org1_regular_user_id, "org1_regular");

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make request - should succeed but with org1 context
    let request = create_auth_request(&token, "/test", Method::GET);
    let response = app.oneshot(request).await.expect("Failed to make request");

    assert_eq!(response.status(), StatusCode::OK);

    // Extract and verify OrgContext shows org1, not org2
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let org_ctx: OrgContext =
        serde_json::from_slice(&body).expect("Failed to deserialize OrgContext");

    // User should always have their own organization context, not be able to impersonate others
    assert_eq!(org_ctx.organization_id, Some(test_data.org1_id));
    assert!(!org_ctx.is_super_admin);
}

#[tokio::test]
async fn test_user_without_organization() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    // Create user without organization assignment
    let no_org_user_id = crate::tests::common::create_test_user(&pool, "no_org_user").await;

    // Create token for user without organization
    let token = create_test_token(&jwt_util, no_org_user_id, "no_org_user");

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make request
    let request = create_auth_request(&token, "/test", Method::GET);
    let response = app.oneshot(request).await.expect("Failed to make request");

    assert_eq!(response.status(), StatusCode::OK);

    // Extract and verify OrgContext
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let org_ctx: OrgContext =
        serde_json::from_slice(&body).expect("Failed to deserialize OrgContext");

    assert_eq!(org_ctx.user_id, no_org_user_id);
    assert_eq!(org_ctx.username, "no_org_user");
    assert_eq!(org_ctx.organization_id, None); // No organization assigned
    assert!(!org_ctx.is_super_admin);
}

#[tokio::test]
async fn test_invalid_token_rejection() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make request with invalid token
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header(header::AUTHORIZATION, "Bearer invalid_token")
        .body(Body::empty())
        .expect("Failed to create test request");

    let response = app.oneshot(request).await.expect("Failed to make request");

    // Should be unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_missing_token_rejection() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make request without token
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .body(Body::empty())
        .expect("Failed to create test request");

    let response = app.oneshot(request).await.expect("Failed to make request");

    // Should be unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_org_context_persistence() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create org admin token
    let token = create_test_token(&jwt_util, test_data.org2_admin_user_id, "org2_admin");

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Make multiple requests to ensure context consistency
    for _ in 0..3 {
        let request = create_auth_request(&token, "/test", Method::GET);
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("Failed to make request");

        assert_eq!(response.status(), StatusCode::OK);

        // Extract and verify OrgContext is consistent
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body");

        let org_ctx: OrgContext =
            serde_json::from_slice(&body).expect("Failed to deserialize OrgContext");

        assert_eq!(org_ctx.user_id, test_data.org2_admin_user_id);
        assert_eq!(org_ctx.username, "org2_admin");
        assert_eq!(org_ctx.organization_id, Some(test_data.org2_id));
        assert!(!org_ctx.is_super_admin);
    }
}

#[tokio::test]
async fn test_organization_user_isolation() {
    let pool = create_test_db().await;
    let casbin_service = create_test_casbin_service().await;
    let jwt_util = Arc::new(JwtUtil::new("test_secret", "24h"));

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test that users from different organizations have different contexts
    let org1_token = create_test_token(&jwt_util, test_data.org1_regular_user_id, "org1_regular");
    let org2_token = create_test_token(&jwt_util, test_data.org2_regular_user_id, "org2_regular");

    // Create auth state
    let auth_state = AuthState {
        jwt_util: jwt_util.clone(),
        casbin_service: casbin_service.clone(),
        db: pool.clone(),
    };

    // Create test app with middleware
    let app = axum::Router::new()
        .route("/test", axum::routing::get(mock_handler))
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

    // Test org1 user
    let org1_request = create_auth_request(&org1_token, "/test", Method::GET);
    let org1_response = app
        .clone()
        .oneshot(org1_request)
        .await
        .expect("Failed to make request");

    let org1_body = axum::body::to_bytes(org1_response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let org1_ctx: OrgContext =
        serde_json::from_slice(&org1_body).expect("Failed to deserialize OrgContext");

    // Test org2 user
    let org2_request = create_auth_request(&org2_token, "/test", Method::GET);
    let org2_response = app
        .oneshot(org2_request)
        .await
        .expect("Failed to make request");

    let org2_body = axum::body::to_bytes(org2_response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");

    let org2_ctx: OrgContext =
        serde_json::from_slice(&org2_body).expect("Failed to deserialize OrgContext");

    // Verify contexts are different and properly isolated
    assert_eq!(org1_ctx.organization_id, Some(test_data.org1_id));
    assert_eq!(org2_ctx.organization_id, Some(test_data.org2_id));
    assert_ne!(org1_ctx.organization_id, org2_ctx.organization_id);
    assert!(!org1_ctx.is_super_admin);
    assert!(!org2_ctx.is_super_admin);
}
