//! LLM Service Unit Tests
//!
//! Tests for LLM provider CRUD operations and service functionality.

use super::*;
use sqlx::SqlitePool;

/// Create an in-memory SQLite database with LLM tables
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    
    // Create LLM tables
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS llm_providers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            api_base TEXT NOT NULL,
            model_name TEXT NOT NULL,
            api_key_encrypted TEXT,
            is_active BOOLEAN NOT NULL DEFAULT FALSE,
            max_tokens INTEGER NOT NULL DEFAULT 4096,
            temperature REAL NOT NULL DEFAULT 0.3,
            timeout_seconds INTEGER NOT NULL DEFAULT 60,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            priority INTEGER NOT NULL DEFAULT 100,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create llm_providers table");
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS llm_analysis_sessions (
            id TEXT PRIMARY KEY,
            provider_id INTEGER,
            scenario TEXT NOT NULL,
            query_id TEXT NOT NULL,
            cluster_id INTEGER,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            completed_at TIMESTAMP,
            input_tokens INTEGER,
            output_tokens INTEGER,
            latency_ms INTEGER,
            error_message TEXT,
            retry_count INTEGER NOT NULL DEFAULT 0
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create llm_analysis_sessions table");
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS llm_analysis_requests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            request_json TEXT NOT NULL,
            sql_hash TEXT NOT NULL,
            profile_hash TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create llm_analysis_requests table");
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS llm_analysis_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            root_causes_json TEXT NOT NULL,
            causal_chains_json TEXT NOT NULL,
            recommendations_json TEXT NOT NULL,
            summary TEXT NOT NULL,
            hidden_issues_json TEXT NOT NULL,
            confidence_avg REAL,
            root_cause_count INTEGER,
            recommendation_count INTEGER,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create llm_analysis_results table");
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS llm_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            cache_key TEXT NOT NULL UNIQUE,
            scenario TEXT NOT NULL,
            request_hash TEXT NOT NULL,
            response_json TEXT NOT NULL,
            hit_count INTEGER NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            expires_at TIMESTAMP NOT NULL,
            last_accessed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create llm_cache table");
    
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS llm_usage_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            provider_id INTEGER,
            total_requests INTEGER NOT NULL DEFAULT 0,
            successful_requests INTEGER NOT NULL DEFAULT 0,
            failed_requests INTEGER NOT NULL DEFAULT 0,
            total_input_tokens INTEGER NOT NULL DEFAULT 0,
            total_output_tokens INTEGER NOT NULL DEFAULT 0,
            avg_latency_ms REAL,
            cache_hits INTEGER NOT NULL DEFAULT 0,
            estimated_cost_usd REAL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(date, provider_id)
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create llm_usage_stats table");
    
    pool
}

/// Create a test provider request
fn create_test_provider_request(name: &str) -> CreateProviderRequest {
    CreateProviderRequest {
        name: name.to_string(),
        display_name: format!("{} Display", name),
        api_base: "https://api.test.com/v1".to_string(),
        model_name: "gpt-4".to_string(),
        api_key: "sk-test-key-12345".to_string(),
        max_tokens: 4096,
        temperature: 0.7,
        timeout_seconds: 60,
        priority: 100,
    }
}

// ============================================================================
// Repository Tests
// ============================================================================

mod repository_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_create_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let req = create_test_provider_request("openai");
        let provider = repo.create_provider(req).await.expect("Failed to create provider");
        
        assert_eq!(provider.name, "openai");
        assert_eq!(provider.display_name, "openai Display");
        assert_eq!(provider.model_name, "gpt-4");
        assert!(!provider.is_active);
        assert!(provider.enabled);
    }
    
    #[tokio::test]
    async fn test_list_providers() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        // Create multiple providers
        repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        repo.create_provider(create_test_provider_request("deepseek")).await.unwrap();
        
        let providers = repo.list_providers().await.expect("Failed to list providers");
        assert_eq!(providers.len(), 2);
    }
    
    #[tokio::test]
    async fn test_get_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let created = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        let fetched = repo.get_provider(created.id).await.expect("Failed to get provider");
        
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "openai");
    }
    
    #[tokio::test]
    async fn test_get_provider_not_found() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let result = repo.get_provider(9999).await.expect("Failed to query");
        assert!(result.is_none());
    }
    
    #[tokio::test]
    async fn test_update_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let created = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        
        let update_req = UpdateProviderRequest {
            display_name: Some("Updated OpenAI".to_string()),
            api_base: None,
            model_name: Some("gpt-4o".to_string()),
            api_key: None,
            max_tokens: Some(8192),
            temperature: None,
            timeout_seconds: None,
            priority: None,
            enabled: None,
        };
        
        let updated = repo.update_provider(created.id, update_req).await.expect("Failed to update");
        
        assert_eq!(updated.display_name, "Updated OpenAI");
        assert_eq!(updated.model_name, "gpt-4o");
        assert_eq!(updated.max_tokens, 8192);
        // Unchanged fields
        assert_eq!(updated.api_base, "https://api.test.com/v1");
    }
    
    #[tokio::test]
    async fn test_activate_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let p1 = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        let p2 = repo.create_provider(create_test_provider_request("deepseek")).await.unwrap();
        
        // Activate first provider
        repo.activate_provider(p1.id).await.expect("Failed to activate");
        
        let active = repo.get_active_provider().await.expect("Failed to get active");
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, p1.id);
        
        // Activate second provider (should deactivate first)
        repo.activate_provider(p2.id).await.expect("Failed to activate");
        
        let active = repo.get_active_provider().await.expect("Failed to get active");
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, p2.id);
        
        // Verify first is no longer active
        let p1_updated = repo.get_provider(p1.id).await.unwrap().unwrap();
        assert!(!p1_updated.is_active);
    }
    
    #[tokio::test]
    async fn test_deactivate_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        repo.activate_provider(provider.id).await.unwrap();
        
        // Verify active
        let active = repo.get_active_provider().await.unwrap();
        assert!(active.is_some());
        
        // Deactivate
        repo.deactivate_provider(provider.id).await.expect("Failed to deactivate");
        
        // Verify no active provider
        let active = repo.get_active_provider().await.unwrap();
        assert!(active.is_none());
    }
    
    #[tokio::test]
    async fn test_delete_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        
        repo.delete_provider(provider.id).await.expect("Failed to delete");
        
        let result = repo.get_provider(provider.id).await.unwrap();
        assert!(result.is_none());
    }
    
    #[tokio::test]
    async fn test_delete_active_provider_fails() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        repo.activate_provider(provider.id).await.unwrap();
        
        let result = repo.delete_provider(provider.id).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_set_provider_enabled() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        assert!(provider.enabled);
        
        // Disable
        let updated = repo.set_provider_enabled(provider.id, false).await.expect("Failed to disable");
        assert!(!updated.enabled);
        
        // Enable
        let updated = repo.set_provider_enabled(provider.id, true).await.expect("Failed to enable");
        assert!(updated.enabled);
    }
    
    #[tokio::test]
    async fn test_disable_active_provider_deactivates() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        repo.activate_provider(provider.id).await.unwrap();
        
        // Verify active
        let active = repo.get_active_provider().await.unwrap();
        assert!(active.is_some());
        
        // Disable (should also deactivate)
        repo.set_provider_enabled(provider.id, false).await.unwrap();
        
        // Verify no active provider
        let active = repo.get_active_provider().await.unwrap();
        assert!(active.is_none());
    }
}

// ============================================================================
// Service Tests
// ============================================================================

mod service_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_service_create_provider() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);
        
        let req = create_test_provider_request("openai");
        let provider = service.create_provider(req).await.expect("Failed to create provider");
        
        assert_eq!(provider.name, "openai");
    }
    
    #[tokio::test]
    async fn test_service_list_providers() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);
        
        service.create_provider(create_test_provider_request("openai")).await.unwrap();
        service.create_provider(create_test_provider_request("deepseek")).await.unwrap();
        
        let providers = service.list_providers().await.expect("Failed to list");
        assert_eq!(providers.len(), 2);
        
        // Verify sensitive data is masked
        for p in &providers {
            if let Some(masked) = &p.api_key_masked {
                assert!(masked.contains("...") || masked == "****");
            }
        }
    }
    
    #[tokio::test]
    async fn test_service_get_provider() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);
        
        let created = service.create_provider(create_test_provider_request("openai")).await.unwrap();
        let fetched = service.get_provider(created.id).await.expect("Failed to get");
        
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "openai");
    }
    
    #[tokio::test]
    async fn test_service_update_provider() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);
        
        let created = service.create_provider(create_test_provider_request("openai")).await.unwrap();
        
        let update = UpdateProviderRequest {
            display_name: Some("New Name".to_string()),
            api_base: None,
            model_name: None,
            api_key: None,
            max_tokens: None,
            temperature: None,
            timeout_seconds: None,
            priority: None,
            enabled: None,
        };
        
        let updated = service.update_provider(created.id, update).await.expect("Failed to update");
        assert_eq!(updated.display_name, "New Name");
    }
    
    #[tokio::test]
    async fn test_service_activate_deactivate() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);
        
        let provider = service.create_provider(create_test_provider_request("openai")).await.unwrap();
        
        service.activate_provider(provider.id).await.expect("Failed to activate");
        let active = service.get_active_provider().await.expect("Failed to get active");
        assert!(active.is_some());
        
        service.deactivate_provider(provider.id).await.expect("Failed to deactivate");
        let active = service.get_active_provider().await.expect("Failed to get active");
        assert!(active.is_none());
    }
    
    #[tokio::test]
    async fn test_service_delete_provider() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);
        
        let provider = service.create_provider(create_test_provider_request("openai")).await.unwrap();
        
        service.delete_provider(provider.id).await.expect("Failed to delete");
        
        let result = service.get_provider(provider.id).await.expect("Failed to query");
        assert!(result.is_none());
    }
    
    #[tokio::test]
    async fn test_service_is_available() {
        let pool = setup_test_db().await;
        
        let enabled_service = LLMServiceImpl::new(pool.clone(), true, 24);
        assert!(enabled_service.is_available());
        
        let disabled_service = LLMServiceImpl::new(pool, false, 24);
        assert!(!disabled_service.is_available());
    }
}

// ============================================================================
// Model Tests
// ============================================================================

mod model_tests {
    use super::*;
    
    #[test]
    fn test_provider_info_masks_api_key() {
        let provider = LLMProvider {
            id: 1,
            name: "test".to_string(),
            display_name: "Test".to_string(),
            api_base: "https://api.test.com".to_string(),
            model_name: "gpt-4".to_string(),
            api_key_encrypted: Some("sk-1234567890abcdef".to_string()),
            is_active: false,
            max_tokens: 4096,
            temperature: 0.7,
            timeout_seconds: 60,
            enabled: true,
            priority: 100,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let info = LLMProviderInfo::from(&provider);
        
        assert!(info.api_key_masked.is_some());
        let masked = info.api_key_masked.unwrap();
        assert!(masked.contains("..."));
        assert!(!masked.contains("1234567890"));
    }
    
    #[test]
    fn test_provider_info_short_key_masked() {
        let provider = LLMProvider {
            id: 1,
            name: "test".to_string(),
            display_name: "Test".to_string(),
            api_base: "https://api.test.com".to_string(),
            model_name: "gpt-4".to_string(),
            api_key_encrypted: Some("short".to_string()),
            is_active: false,
            max_tokens: 4096,
            temperature: 0.7,
            timeout_seconds: 60,
            enabled: true,
            priority: 100,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let info = LLMProviderInfo::from(&provider);
        assert_eq!(info.api_key_masked, Some("****".to_string()));
    }
    
    #[test]
    fn test_llm_scenario_as_str() {
        assert_eq!(LLMScenario::RootCauseAnalysis.as_str(), "root_cause_analysis");
        assert_eq!(LLMScenario::SqlOptimization.as_str(), "sql_optimization");
    }
    
    #[test]
    fn test_session_status_conversion() {
        assert_eq!(SessionStatus::Pending.as_str(), "pending");
        assert_eq!(SessionStatus::from_str("completed"), SessionStatus::Completed);
        assert_eq!(SessionStatus::from_str("unknown"), SessionStatus::Failed);
    }
    
    #[test]
    fn test_llm_error_is_retryable() {
        assert!(LLMError::Timeout(30).is_retryable());
        assert!(LLMError::RateLimited(60).is_retryable());
        assert!(LLMError::ApiError("test".to_string()).is_retryable());
        assert!(!LLMError::Disabled.is_retryable());
        assert!(!LLMError::NoProviderConfigured.is_retryable());
    }
}

// ============================================================================
// Cache Tests
// ============================================================================

mod cache_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cache_response() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let cache_key = "test_cache_key";
        let response_json = r#"{"result": "test"}"#;
        
        repo.cache_response(
            cache_key,
            LLMScenario::RootCauseAnalysis,
            "sql_hash",
            response_json,
            24,
        ).await.expect("Failed to cache");
        
        let cached = repo.get_cached_response(cache_key).await.expect("Failed to get cache");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), response_json);
    }
    
    #[tokio::test]
    async fn test_cache_miss() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let cached = repo.get_cached_response("nonexistent").await.expect("Failed to query");
        assert!(cached.is_none());
    }
    
    #[tokio::test]
    async fn test_clean_expired_cache() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        // Insert expired cache entry directly
        sqlx::query(
            r#"INSERT INTO llm_cache (cache_key, scenario, request_hash, response_json, expires_at)
               VALUES ('expired', 'test', 'hash', '{}', datetime('now', '-1 hour'))"#
        )
        .execute(&repo.pool)
        .await
        .unwrap();
        
        let deleted = repo.clean_expired_cache().await.expect("Failed to clean");
        assert_eq!(deleted, 1);
    }
}

// ============================================================================
// Session Tests
// ============================================================================

mod session_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_create_session() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        // First create a provider
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        
        let session_id = repo.create_session(
            "query_123",
            provider.id,
            Some(1),
            LLMScenario::RootCauseAnalysis,
        ).await.expect("Failed to create session");
        
        assert!(!session_id.is_empty());
        
        let session = repo.get_session(&session_id).await.expect("Failed to get session");
        assert!(session.is_some());
        let session = session.unwrap();
        assert_eq!(session.query_id, "query_123");
        assert_eq!(session.status, "pending");
    }
    
    #[tokio::test]
    async fn test_update_session_status() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        let session_id = repo.create_session("query_123", provider.id, None, LLMScenario::RootCauseAnalysis).await.unwrap();
        
        repo.update_session_status(&session_id, SessionStatus::Processing).await.expect("Failed to update");
        
        let session = repo.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.status, "processing");
    }
    
    #[tokio::test]
    async fn test_complete_session() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        let session_id = repo.create_session("query_123", provider.id, None, LLMScenario::RootCauseAnalysis).await.unwrap();
        
        repo.complete_session(
            &session_id,
            SessionStatus::Completed,
            100,
            200,
            1500,
            None,
        ).await.expect("Failed to complete session");
        
        let session = repo.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.status, "completed");
        assert_eq!(session.input_tokens, Some(100));
        assert_eq!(session.output_tokens, Some(200));
        assert_eq!(session.latency_ms, Some(1500));
    }
}

// ============================================================================
// Usage Stats Tests
// ============================================================================

mod usage_stats_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_record_usage() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);
        
        let provider = repo.create_provider(create_test_provider_request("openai")).await.unwrap();
        
        repo.record_usage(provider.id, 100, 50, true, 500, false).await.expect("Failed to record");
        repo.record_usage(provider.id, 200, 100, true, 600, true).await.expect("Failed to record");
        repo.record_usage(provider.id, 50, 0, false, 100, false).await.expect("Failed to record");
        
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let stats = repo.get_usage_stats(&today, &today).await.expect("Failed to get stats");
        
        assert_eq!(stats.len(), 1);
        let stat = &stats[0];
        assert_eq!(stat.total_requests, 3);
        assert_eq!(stat.successful_requests, 2);
        assert_eq!(stat.failed_requests, 1);
        assert_eq!(stat.cache_hits, 1);
    }
}
