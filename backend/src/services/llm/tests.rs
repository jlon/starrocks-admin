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
        "#,
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
        "#,
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
        "#,
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
        "#,
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
        "#,
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
        "#,
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
        let provider = repo
            .create_provider(req)
            .await
            .expect("Failed to create provider");

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
        repo.create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        repo.create_provider(create_test_provider_request("deepseek"))
            .await
            .unwrap();

        let providers = repo
            .list_providers()
            .await
            .expect("Failed to list providers");
        assert_eq!(providers.len(), 2);
    }

    #[tokio::test]
    async fn test_get_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let created = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        let fetched = repo
            .get_provider(created.id)
            .await
            .expect("Failed to get provider");

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

        let created = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();

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

        let updated = repo
            .update_provider(created.id, update_req)
            .await
            .expect("Failed to update");

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

        let p1 = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        let p2 = repo
            .create_provider(create_test_provider_request("deepseek"))
            .await
            .unwrap();

        // Activate first provider
        repo.activate_provider(p1.id)
            .await
            .expect("Failed to activate");

        let active = repo
            .get_active_provider()
            .await
            .expect("Failed to get active");
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, p1.id);

        // Activate second provider (should deactivate first)
        repo.activate_provider(p2.id)
            .await
            .expect("Failed to activate");

        let active = repo
            .get_active_provider()
            .await
            .expect("Failed to get active");
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

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        repo.activate_provider(provider.id).await.unwrap();

        // Verify active
        let active = repo.get_active_provider().await.unwrap();
        assert!(active.is_some());

        // Deactivate
        repo.deactivate_provider(provider.id)
            .await
            .expect("Failed to deactivate");

        // Verify no active provider
        let active = repo.get_active_provider().await.unwrap();
        assert!(active.is_none());
    }

    #[tokio::test]
    async fn test_delete_provider() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();

        repo.delete_provider(provider.id)
            .await
            .expect("Failed to delete");

        let result = repo.get_provider(provider.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_active_provider_fails() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        repo.activate_provider(provider.id).await.unwrap();

        let result = repo.delete_provider(provider.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_provider_enabled() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        assert!(provider.enabled);

        // Disable
        let updated = repo
            .set_provider_enabled(provider.id, false)
            .await
            .expect("Failed to disable");
        assert!(!updated.enabled);

        // Enable
        let updated = repo
            .set_provider_enabled(provider.id, true)
            .await
            .expect("Failed to enable");
        assert!(updated.enabled);
    }

    #[tokio::test]
    async fn test_disable_active_provider_deactivates() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
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
        let provider = service
            .create_provider(req)
            .await
            .expect("Failed to create provider");

        assert_eq!(provider.name, "openai");
    }

    #[tokio::test]
    async fn test_service_list_providers() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);

        service
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        service
            .create_provider(create_test_provider_request("deepseek"))
            .await
            .unwrap();

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

        let created = service
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        let fetched = service
            .get_provider(created.id)
            .await
            .expect("Failed to get");

        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "openai");
    }

    #[tokio::test]
    async fn test_service_update_provider() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);

        let created = service
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();

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

        let updated = service
            .update_provider(created.id, update)
            .await
            .expect("Failed to update");
        assert_eq!(updated.display_name, "New Name");
    }

    #[tokio::test]
    async fn test_service_activate_deactivate() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);

        let provider = service
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();

        service
            .activate_provider(provider.id)
            .await
            .expect("Failed to activate");
        let active = service
            .get_active_provider()
            .await
            .expect("Failed to get active");
        assert!(active.is_some());

        service
            .deactivate_provider(provider.id)
            .await
            .expect("Failed to deactivate");
        let active = service
            .get_active_provider()
            .await
            .expect("Failed to get active");
        assert!(active.is_none());
    }

    #[tokio::test]
    async fn test_service_delete_provider() {
        let pool = setup_test_db().await;
        let service = LLMServiceImpl::new(pool, true, 24);

        let provider = service
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();

        service
            .delete_provider(provider.id)
            .await
            .expect("Failed to delete");

        let result = service
            .get_provider(provider.id)
            .await
            .expect("Failed to query");
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
        )
        .await
        .expect("Failed to cache");

        let cached = repo
            .get_cached_response(cache_key)
            .await
            .expect("Failed to get cache");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), response_json);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let cached = repo
            .get_cached_response("nonexistent")
            .await
            .expect("Failed to query");
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_clean_expired_cache() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        // Insert expired cache entry directly
        sqlx::query(
            r#"INSERT INTO llm_cache (cache_key, scenario, request_hash, response_json, expires_at)
               VALUES ('expired', 'test', 'hash', '{}', datetime('now', '-1 hour'))"#,
        )
        .execute(repo.pool())
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
        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();

        let session_id = repo
            .create_session("query_123", provider.id, Some(1), LLMScenario::RootCauseAnalysis)
            .await
            .expect("Failed to create session");

        assert!(!session_id.is_empty());

        let session = repo
            .get_session(&session_id)
            .await
            .expect("Failed to get session");
        assert!(session.is_some());
        let session = session.unwrap();
        assert_eq!(session.query_id, "query_123");
        assert_eq!(session.status, "pending");
    }

    #[tokio::test]
    async fn test_update_session_status() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        let session_id = repo
            .create_session("query_123", provider.id, None, LLMScenario::RootCauseAnalysis)
            .await
            .unwrap();

        repo.update_session_status(&session_id, SessionStatus::Processing)
            .await
            .expect("Failed to update");

        let session = repo.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.status, "processing");
    }

    #[tokio::test]
    async fn test_complete_session() {
        let pool = setup_test_db().await;
        let repo = LLMRepository::new(pool);

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();
        let session_id = repo
            .create_session("query_123", provider.id, None, LLMScenario::RootCauseAnalysis)
            .await
            .unwrap();

        repo.complete_session(&session_id, SessionStatus::Completed, 100, 200, 1500, None)
            .await
            .expect("Failed to complete session");

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

        let provider = repo
            .create_provider(create_test_provider_request("openai"))
            .await
            .unwrap();

        repo.record_usage(provider.id, 100, 50, true, 500, false)
            .await
            .expect("Failed to record");
        repo.record_usage(provider.id, 200, 100, true, 600, true)
            .await
            .expect("Failed to record");
        repo.record_usage(provider.id, 50, 0, false, 100, false)
            .await
            .expect("Failed to record");

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let stats = repo
            .get_usage_stats(&today, &today)
            .await
            .expect("Failed to get stats");

        assert_eq!(stats.len(), 1);
        let stat = &stats[0];
        assert_eq!(stat.total_requests, 3);
        assert_eq!(stat.successful_requests, 2);
        assert_eq!(stat.failed_requests, 1);
        assert_eq!(stat.cache_hits, 1);
    }
}

// ============================================================================
// LLM Integration Test with Real Profile
// ============================================================================

// Use the determine_table_type and determine_connector_type from root_cause module
use super::scenarios::root_cause::{determine_connector_type, determine_table_type};

mod llm_integration_tests {
    use super::*;
    use crate::services::profile_analyzer::{
        AggregatedDiagnostic, AnalysisContext, LLMCausalChain, LLMEnhancedAnalysis, LLMHiddenIssue,
        MergedRecommendation, MergedRootCause, ProfileAnalysisResponse,
        analyze_profile_with_context,
    };
    use std::collections::HashMap;
    use std::fs;

    /// Test the complete LLM integration pipeline with profile12
    ///
    /// Run with: cargo test llm_integration_tests::test_profile12_llm_integration --lib -- --nocapture --ignored
    #[tokio::test]
    #[ignore] // Run manually with --ignored flag
    async fn test_profile12_llm_integration() {
        let sep = "=".repeat(80);
        println!("\n{}", sep);
        println!("🧪 LLM Integration Test - prrofile12.txt");
        println!("{}\n", sep);

        // Step 1: Read profile file
        let profile_path = "tests/fixtures/profiles/prrofile12.txt";
        let profile_content =
            fs::read_to_string(profile_path).expect("Failed to read profile file");

        println!("📄 Profile loaded: {} bytes\n", profile_content.len());

        // Step 2: Run rule engine analysis (骨架)
        println!("{}", sep);
        println!("🦴 Step 1: Rule Engine Analysis (骨架)");
        println!("{}\n", sep);

        let context = AnalysisContext { cluster_variables: None, cluster_id: None };
        let response = analyze_profile_with_context(&profile_content, &context)
            .expect("Failed to analyze profile");

        // Print summary
        println!("📊 Summary:");
        if let Some(summary) = &response.summary {
            println!("   - Query ID: {}", summary.query_id);
            println!("   - Query State: {}", summary.query_state);
            println!("   - Total Time: {:?} ms", summary.total_time_ms);
            let sql_preview = if summary.sql_statement.len() > 200 {
                format!("{}...", &summary.sql_statement[..200])
            } else {
                summary.sql_statement.clone()
            };
            println!("   - SQL (truncated): {}", sql_preview);
        }

        println!("\n📋 Aggregated Diagnostics ({} issues):", response.aggregated_diagnostics.len());
        for (i, diag) in response.aggregated_diagnostics.iter().enumerate() {
            println!(
                "   {}. [{}] {} - {} ({} nodes)",
                i + 1,
                diag.rule_id,
                diag.severity,
                diag.message,
                diag.node_count
            );
            let reason_preview = truncate_str(&diag.reason, 150);
            println!("      Reason: {}", reason_preview);
            if !diag.suggestions.is_empty() {
                for s in &diag.suggestions {
                    println!("      💡 {}", s);
                }
            }
        }

        println!("\n🎯 Performance Score: {:.1}", response.performance_score);

        // Step 3: Build LLM request
        println!("\n{}", sep);
        println!("📤 Step 2: Data Sent to LLM");
        println!("{}\n", sep);

        let llm_request = build_llm_request_for_test(&response);
        let request_json = serde_json::to_string_pretty(&llm_request).unwrap();
        println!("{}", request_json);

        // Step 4: Connect to real database and call LLM
        println!("\n{}", sep);
        println!("🤖 Step 3: LLM Response (Real OpenRouter API)");
        println!("{}\n", sep);

        // Try multiple possible database paths
        let db_paths = [
            "data/starrocks-admin.db",
            "starrocks_admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/data/starrocks-admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/starrocks_admin.db",
        ];
        let db_path = db_paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .expect("Database not found. Run backend first to initialize.");
        println!("📁 Using database: {}", db_path);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", db_path))
            .await
            .expect("Failed to connect to database");

        let llm_service = LLMServiceImpl::new(pool, true, 24);

        if !llm_service.is_available() {
            println!("⚠️  No active LLM provider found.");
            println!("    Please activate a provider first via the API or UI.");
            return;
        }

        println!("✅ LLM service available, calling OpenRouter API...\n");

        let query_id = response
            .summary
            .as_ref()
            .map(|s| s.query_id.clone())
            .unwrap_or_else(|| "test-query".to_string());

        let start = std::time::Instant::now();
        let llm_result = llm_service
            .analyze(&llm_request, &query_id, None, false)
            .await;
        let elapsed = start.elapsed();

        match llm_result {
            Ok(result) => {
                let llm_response = result.response;
                println!("⏱️  LLM call took: {:?} (from_cache: {})\n", elapsed, result.from_cache);
                println!("📥 LLM Response:");
                println!("{}", serde_json::to_string_pretty(&llm_response).unwrap());

                // Step 5: Merge results
                println!("\n{}", sep);
                println!("🔄 Step 4: Merged Result (骨架 + 血肉)");
                println!("{}\n", sep);

                let merged =
                    merge_results_for_test(&response.aggregated_diagnostics, &llm_response);
                println!("{}", serde_json::to_string_pretty(&merged).unwrap());

                // Print summary
                println!("\n{}", sep);
                println!("📊 Final Summary");
                println!("{}\n", sep);
                println!(
                    "🦴 Rule Engine (骨架): {} diagnostics",
                    response.aggregated_diagnostics.len()
                );
                println!(
                    "🩸 LLM (血肉): {} root causes, {} recommendations",
                    llm_response.root_causes.len(),
                    llm_response.recommendations.len()
                );
                println!(
                    "🔄 Merged: {} root causes, {} recommendations",
                    merged.root_causes.len(),
                    merged.merged_recommendations.len()
                );
            },
            Err(e) => {
                println!("❌ LLM call failed after {:?}: {}", elapsed, e);
            },
        }
    }

    /// Build LLM request from profile analysis response - ENHANCED VERSION
    /// Now includes full SQL and raw profile data for deep analysis
    fn build_llm_request_for_test(response: &ProfileAnalysisResponse) -> RootCauseAnalysisRequest {
        use crate::services::llm::scenarios::root_cause::{
            AggDetailForLLM, ExchangeDetailForLLM, JoinDetailForLLM, OperatorDetailForLLM,
            ProfileDataForLLM, ScanDetailForLLM, TimeDistributionForLLM,
        };

        let summary = response.summary.as_ref();

        // CHANGE 1: Full SQL without truncation
        let sql = summary.map(|s| s.sql_statement.clone()).unwrap_or_default();

        let query_summary = QuerySummaryForLLM {
            sql_statement: sql.clone(),
            query_type: summary
                .and_then(|s| s.query_type.clone())
                .unwrap_or_else(|| "SELECT".to_string()),
            query_complexity: Some(format!(
                "{:?}",
                crate::services::profile_analyzer::analyzer::QueryComplexity::from_sql(&sql)
            )),
            total_time_seconds: summary
                .map(|s| s.total_time_ms.unwrap_or(0.0) / 1000.0)
                .unwrap_or(0.0),
            scan_bytes: summary.and_then(|s| s.total_bytes_read).unwrap_or(0),
            output_rows: summary.and_then(|s| s.result_rows).unwrap_or(0),
            be_count: summary
                .and_then(|s| s.total_instance_count.map(|c| c as u32))
                .unwrap_or(3),
            has_spill: summary
                .and_then(|s| {
                    s.query_spill_bytes
                        .as_ref()
                        .map(|b| !b.is_empty() && b != "0" && b != "0.000 B")
                })
                .unwrap_or(false),
            spill_bytes: summary.and_then(|s| s.query_spill_bytes.clone()),
            session_variables: HashMap::new(),
        };

        // CHANGE 2: Build rich profile data from execution tree
        let profile_data = response.execution_tree.as_ref().map(|tree| {
            // All operators with full metrics
            let operators: Vec<OperatorDetailForLLM> = tree
                .nodes
                .iter()
                .map(|n| OperatorDetailForLLM {
                    operator: n.operator_name.clone(),
                    plan_node_id: n.plan_node_id.unwrap_or(-1),
                    time_pct: n.time_percentage.unwrap_or(0.0),
                    rows: n.rows.unwrap_or(0),
                    estimated_rows: None, // TODO: extract from plan
                    memory_bytes: parse_bytes(
                        &n.unique_metrics
                            .get("PeakMemoryUsage")
                            .cloned()
                            .unwrap_or_default(),
                    ),
                    metrics: n
                        .unique_metrics
                        .iter()
                        .filter(|(k, _)| is_important_metric(k))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                })
                .collect();

            // Scan details - determine table type from CATALOG, not scan operator!
            // Key insight:
            // - default_catalog.db.table or db.table → internal (StarRocks native table)
            // - hive_catalog.db.table, iceberg_catalog.db.table → external (foreign table)
            // SCAN operator type (OLAP_SCAN vs CONNECTOR_SCAN) indicates storage architecture,
            // NOT whether the table is internal or external!
            let scan_details: Vec<ScanDetailForLLM> = tree
                .nodes
                .iter()
                .filter(|n| n.operator_name.contains("SCAN"))
                .map(|n| {
                    let metrics = &n.unique_metrics;
                    let scan_type = n.operator_name.clone();

                    // Get full table name (may include catalog.database.table)
                    let table_name = metrics
                        .get("Table")
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());

                    // Determine table type from CATALOG prefix, not scan operator!
                    // - "default_catalog.db.table" or "db.table" (no catalog) → internal
                    // - "hive_catalog.db.table", "iceberg_catalog.xxx" → external
                    let table_type = determine_table_type(&table_name);

                    // Determine connector type from profile metrics
                    // For external tables: hive, iceberg, hudi, deltalake, paimon, jdbc, es
                    // For internal tables: native
                    let connector_type = if table_type == "external" {
                        Some(determine_connector_type(&metrics))
                    } else {
                        Some("native".to_string())
                    };

                    ScanDetailForLLM {
                        plan_node_id: n.plan_node_id.unwrap_or(-1),
                        table_name: table_name.clone(),
                        scan_type,
                        table_type,
                        connector_type,
                        rows_read: parse_number(
                            metrics.get("RawRowsRead").or(metrics.get("RowsRead")),
                        ),
                        rows_returned: n.rows.unwrap_or(0),
                        filter_ratio: {
                            let read = parse_number(
                                metrics.get("RawRowsRead").or(metrics.get("RowsRead")),
                            );
                            let ret = n.rows.unwrap_or(0);
                            if read > 0 { 1.0 - (ret as f64 / read as f64) } else { 0.0 }
                        },
                        scan_ranges: parse_number_opt(metrics.get("ScanRanges")),
                        bytes_read: parse_bytes(
                            metrics
                                .get("BytesRead")
                                .or(metrics.get("CompressedBytesRead"))
                                .cloned()
                                .as_ref()
                                .unwrap_or(&String::new()),
                        ),
                        io_time_ms: parse_time_ms(
                            metrics.get("IOTaskWaitTime").or(metrics.get("ScanTime")),
                        ),
                        cache_hit_rate: parse_percentage(metrics.get("DataCacheHitRate")),
                        predicates: metrics.get("Predicates").cloned(),
                        partitions_scanned: metrics
                            .get("PartitionsScanned")
                            .or(metrics.get("TabletCount"))
                            .cloned(),
                        full_table_path: if table_name.contains(".") {
                            Some(table_name)
                        } else {
                            None
                        },
                    }
                })
                .collect();

            // Join details
            let join_details: Vec<JoinDetailForLLM> = tree
                .nodes
                .iter()
                .filter(|n| n.operator_name.contains("JOIN"))
                .map(|n| {
                    let metrics = &n.unique_metrics;
                    JoinDetailForLLM {
                        plan_node_id: n.plan_node_id.unwrap_or(-1),
                        join_type: n.operator_name.clone(),
                        build_rows: parse_number(metrics.get("BuildRows")),
                        probe_rows: parse_number(metrics.get("ProbeRows")),
                        output_rows: n.rows.unwrap_or(0),
                        hash_table_memory: parse_bytes(
                            metrics
                                .get("HashTableMemoryUsage")
                                .cloned()
                                .as_ref()
                                .unwrap_or(&String::new()),
                        ),
                        is_broadcast: metrics
                            .get("JoinType")
                            .map(|t| t.contains("BROADCAST"))
                            .unwrap_or(false),
                        runtime_filter: metrics.get("RuntimeFilterDescription").cloned(),
                    }
                })
                .collect();

            // Aggregation details
            let agg_details: Vec<AggDetailForLLM> = tree
                .nodes
                .iter()
                .filter(|n| n.operator_name.contains("AGGREGAT"))
                .map(|n| {
                    let metrics = &n.unique_metrics;
                    let input =
                        parse_number(metrics.get("InputRows").or(metrics.get("PushRowNum")));
                    let output = n.rows.unwrap_or(0);
                    AggDetailForLLM {
                        plan_node_id: n.plan_node_id.unwrap_or(-1),
                        input_rows: input,
                        output_rows: output,
                        agg_ratio: if input > 0 { output as f64 / input as f64 } else { 1.0 },
                        group_by_keys: metrics
                            .get("GroupByKeys")
                            .or(metrics.get("GroupingKeys"))
                            .cloned(),
                        hash_table_memory: parse_bytes(
                            metrics
                                .get("HashTableMemoryUsage")
                                .cloned()
                                .as_ref()
                                .unwrap_or(&String::new()),
                        ),
                        is_streaming: metrics
                            .get("AggMode")
                            .map(|m| m.contains("STREAMING"))
                            .unwrap_or(false),
                    }
                })
                .collect();

            // Exchange details
            let exchange_details: Vec<ExchangeDetailForLLM> = tree
                .nodes
                .iter()
                .filter(|n| n.operator_name.contains("EXCHANGE"))
                .map(|n| {
                    let metrics = &n.unique_metrics;
                    ExchangeDetailForLLM {
                        plan_node_id: n.plan_node_id.unwrap_or(-1),
                        exchange_type: metrics
                            .get("PartType")
                            .cloned()
                            .unwrap_or_else(|| "SHUFFLE".to_string()),
                        bytes_sent: parse_bytes(
                            metrics
                                .get("BytesSent")
                                .or(metrics.get("NetworkBytes"))
                                .cloned()
                                .as_ref()
                                .unwrap_or(&String::new()),
                        )
                        .unwrap_or(0),
                        rows_sent: parse_number(metrics.get("RowsSent")),
                        network_time_ms: parse_time_ms(
                            metrics
                                .get("NetworkTime")
                                .or(metrics.get("WaitForDataTime")),
                        ),
                    }
                })
                .collect();

            // Time distribution for skew detection
            let time_distribution = {
                let times: Vec<f64> = tree
                    .nodes
                    .iter()
                    .filter_map(|n| n.time_percentage)
                    .filter(|&t| t > 0.0)
                    .collect();
                if times.is_empty() {
                    None
                } else {
                    let max = times.iter().cloned().fold(f64::MIN, f64::max);
                    let min = times.iter().cloned().fold(f64::MAX, f64::min);
                    let avg = times.iter().sum::<f64>() / times.len() as f64;
                    Some(TimeDistributionForLLM {
                        max_time_ms: max,
                        min_time_ms: min,
                        avg_time_ms: avg,
                        skew_ratio: if avg > 0.0 { max / avg } else { 1.0 },
                        per_instance: vec![], // Simplified for now
                    })
                }
            };

            ProfileDataForLLM {
                operators,
                time_distribution,
                scan_details,
                join_details,
                agg_details,
                exchange_details,
            }
        });

        // Execution plan (simplified DAG)
        let dag_description = response
            .execution_tree
            .as_ref()
            .map(|tree| {
                tree.nodes
                    .iter()
                    .take(15)
                    .map(|n| format!("{}({})", n.operator_name, n.plan_node_id.unwrap_or(-1)))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            })
            .unwrap_or_else(|| "Unknown DAG".to_string());

        let hotspot_nodes: Vec<HotspotNodeForLLM> = response
            .execution_tree
            .as_ref()
            .map(|tree| {
                tree.nodes
                    .iter()
                    .filter(|n| n.time_percentage.unwrap_or(0.0) > 5.0)
                    .take(10)
                    .map(|n| HotspotNodeForLLM {
                        operator: n.operator_name.clone(),
                        plan_node_id: n.plan_node_id.unwrap_or(-1),
                        time_percentage: n.time_percentage.unwrap_or(0.0),
                        key_metrics: n
                            .unique_metrics
                            .iter()
                            .filter(|(k, _)| is_important_metric(k))
                            .take(15)
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                        upstream_operators: vec![],
                    })
                    .collect()
            })
            .unwrap_or_default();

        let execution_plan = ExecutionPlanForLLM { dag_description, hotspot_nodes };

        // Rule diagnostics (as reference for LLM)
        let diagnostics: Vec<DiagnosticForLLM> = response
            .aggregated_diagnostics
            .iter()
            .map(|d| DiagnosticForLLM {
                rule_id: d.rule_id.clone(),
                severity: d.severity.clone(),
                operator: d
                    .affected_nodes
                    .first()
                    .map(|s| s.split('/').last().unwrap_or("unknown"))
                    .unwrap_or("unknown")
                    .to_string(),
                plan_node_id: None,
                message: format!("{} ({}个节点)", d.message, d.node_count),
                evidence: {
                    let mut e = HashMap::new();
                    e.insert("reason".to_string(), d.reason.clone());
                    e.insert("affected_nodes".to_string(), d.affected_nodes.join(", "));
                    e
                },
                threshold_info: None,
            })
            .collect();

        RootCauseAnalysisRequest {
            query_summary,
            profile_data,
            execution_plan,
            rule_diagnostics: diagnostics,
            key_metrics: KeyMetricsForLLM::default(),
            user_question: None,
        }
    }

    /// Check if metric is important for LLM analysis
    fn is_important_metric(key: &str) -> bool {
        let important = [
            "Table",
            "Predicates",
            "RowsRead",
            "RawRowsRead",
            "RowsReturned",
            "BytesRead",
            "ScanRanges",
            "TabletCount",
            "PartitionsScanned",
            "IOTaskWaitTime",
            "ScanTime",
            "DataCacheHitRate",
            "DataCacheReadBytes",
            "BuildRows",
            "ProbeRows",
            "HashTableMemoryUsage",
            "JoinType",
            "InputRows",
            "OutputRows",
            "GroupByKeys",
            "AggMode",
            "BytesSent",
            "NetworkTime",
            "PartType",
            "PeakMemoryUsage",
            "SpillBytes",
            "SpillTime",
            "EstimatedRows",
            "ActualRows",
            "CardinalityError",
        ];
        important.iter().any(|&i| key.contains(i))
    }

    /// Parse number from metric string like "1.705B (1704962761)" or "1234"
    fn parse_number(s: Option<&String>) -> u64 {
        s.and_then(|v| {
            // Try to extract number in parentheses first: "1.705B (1704962761)"
            if let Some(start) = v.find('(') {
                if let Some(end) = v.find(')') {
                    return v[start + 1..end].parse().ok();
                }
            }
            // Otherwise try direct parse
            v.replace(",", "").parse().ok()
        })
        .unwrap_or(0)
    }

    fn parse_number_opt(s: Option<&String>) -> Option<u64> {
        let n = parse_number(s);
        if n > 0 { Some(n) } else { None }
    }

    /// Parse bytes from string like "68.750 MB" or "20.597 GB"
    fn parse_bytes(s: &str) -> Option<u64> {
        if s.is_empty() {
            return None;
        }
        let s = s.trim();
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let num: f64 = parts[0].replace(",", "").parse().ok()?;
        let multiplier = if parts.len() > 1 {
            match parts[1].to_uppercase().as_str() {
                "B" | "BYTES" => 1,
                "KB" => 1024_u64,
                "MB" => 1024_u64 * 1024,
                "GB" => 1024_u64 * 1024 * 1024,
                "TB" => 1024_u64 * 1024 * 1024 * 1024,
                _ => 1,
            }
        } else {
            1
        };
        Some((num * multiplier as f64) as u64)
    }

    /// Parse time from string like "1m34s" or "717.077us"
    fn parse_time_ms(s: Option<&String>) -> Option<f64> {
        s.and_then(|v| {
            let v = v.trim();
            if v.contains("ms") {
                v.replace("ms", "").trim().parse().ok()
            } else if v.contains("us") {
                v.replace("us", "")
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|n| n / 1000.0)
            } else if v.contains("ns") {
                v.replace("ns", "")
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|n| n / 1_000_000.0)
            } else if v.contains('m') && v.contains('s') {
                // "1m34s" format
                let parts: Vec<&str> = v.split('m').collect();
                if parts.len() == 2 {
                    let mins: f64 = parts[0].parse().ok()?;
                    let secs: f64 = parts[1].replace("s", "").parse().ok()?;
                    Some((mins * 60.0 + secs) * 1000.0)
                } else {
                    None
                }
            } else if v.ends_with('s') {
                v.replace("s", "")
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|n| n * 1000.0)
            } else {
                v.parse().ok()
            }
        })
    }

    /// Parse percentage from string
    fn parse_percentage(s: Option<&String>) -> Option<f64> {
        s.and_then(|v| v.replace("%", "").trim().parse().ok())
    }

    /// Merge rule diagnostics with LLM response
    fn merge_results_for_test(
        rule_diagnostics: &[AggregatedDiagnostic],
        llm_response: &RootCauseAnalysisResponse,
    ) -> LLMEnhancedAnalysis {
        use std::collections::HashSet;

        let mut root_causes = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();

        // Add LLM root causes first (higher priority)
        for llm_rc in &llm_response.root_causes {
            let id = llm_rc.root_cause_id.clone();
            seen_ids.insert(id.clone());

            let related_rules: Vec<String> = llm_rc
                .symptoms
                .iter()
                .filter(|s| rule_diagnostics.iter().any(|d| &d.rule_id == *s))
                .cloned()
                .collect();

            let source = if related_rules.is_empty() { "llm" } else { "both" };

            root_causes.push(MergedRootCause {
                id,
                related_rule_ids: related_rules,
                description: llm_rc.description.clone(),
                is_implicit: llm_rc.is_implicit,
                confidence: llm_rc.confidence,
                source: source.to_string(),
                evidence: llm_rc.evidence.clone(),
                symptoms: llm_rc.symptoms.clone(),
            });
        }

        // Add uncovered rule diagnostics
        for diag in rule_diagnostics {
            let is_covered = llm_response
                .root_causes
                .iter()
                .any(|rc| rc.symptoms.contains(&diag.rule_id));

            if !is_covered {
                let id = format!("rule_{}", diag.rule_id);
                if !seen_ids.contains(&id) {
                    seen_ids.insert(id.clone());
                    root_causes.push(MergedRootCause {
                        id,
                        related_rule_ids: vec![diag.rule_id.clone()],
                        description: diag.message.clone(),
                        is_implicit: false,
                        confidence: 1.0,
                        source: "rule".to_string(),
                        evidence: vec![diag.reason.clone()],
                        symptoms: vec![],
                    });
                }
            }
        }

        root_causes.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Merge recommendations
        let mut recommendations = Vec::new();
        let mut seen_actions: HashSet<String> = HashSet::new();

        for rec in &llm_response.recommendations {
            let action_key: String = rec
                .action
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            if !seen_actions.contains(&action_key) {
                seen_actions.insert(action_key);
                recommendations.push(MergedRecommendation {
                    priority: rec.priority,
                    action: rec.action.clone(),
                    expected_improvement: rec.expected_improvement.clone(),
                    sql_example: rec.sql_example.clone(),
                    source: "llm".to_string(),
                    related_root_causes: vec![],
                    is_root_cause_fix: true,
                });
            }
        }

        let mut rule_priority = recommendations.len() as u32 + 1;
        for diag in rule_diagnostics {
            for suggestion in &diag.suggestions {
                let action_key: String = suggestion
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect();
                if !seen_actions.contains(&action_key) {
                    seen_actions.insert(action_key);
                    recommendations.push(MergedRecommendation {
                        priority: rule_priority,
                        action: suggestion.clone(),
                        expected_improvement: String::new(),
                        sql_example: None,
                        source: "rule".to_string(),
                        related_root_causes: vec![diag.rule_id.clone()],
                        is_root_cause_fix: false,
                    });
                    rule_priority += 1;
                }
            }
        }

        recommendations.sort_by_key(|r| r.priority);

        LLMEnhancedAnalysis {
            available: true,
            status: "completed".to_string(),
            root_causes,
            causal_chains: llm_response
                .causal_chains
                .iter()
                .map(|c| LLMCausalChain {
                    chain: c.chain.clone(),
                    explanation: c.explanation.clone(),
                })
                .collect(),
            merged_recommendations: recommendations,
            summary: llm_response.summary.clone(),
            hidden_issues: llm_response
                .hidden_issues
                .iter()
                .map(|h| LLMHiddenIssue {
                    issue: h.issue.clone(),
                    suggestion: h.suggestion.clone(),
                })
                .collect(),
            from_cache: false,
            elapsed_time_ms: None,
        }
    }

    /// Safely truncate a string at char boundary
    fn truncate_str(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            s.to_string()
        } else {
            let truncated: String = s.chars().take(max_chars).collect();
            format!("{}...", truncated)
        }
    }
}

// ============================================================================
// Dynamic Prompt Generation Tests
// ============================================================================

mod prompt_generation_tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::services::llm::scenarios::root_cause::{
        DiagnosticForLLM, ExecutionPlanForLLM, KeyMetricsForLLM, ProfileDataForLLM,
        QuerySummaryForLLM, RootCauseAnalysisRequest, ScanDetailForLLM, build_system_prompt,
    };
    use std::collections::HashMap;

    /// Test dynamic prompt generation with internal tables
    #[test]
    fn test_prompt_with_internal_tables() {
        let scan_details = vec![ScanDetailForLLM {
            plan_node_id: 1,
            table_name: "default_catalog.db.orders".to_string(),
            scan_type: "OLAP_SCAN".to_string(),
            table_type: "internal".to_string(),
            connector_type: Some("native".to_string()),
            rows_read: 1000000,
            rows_returned: 50000,
            filter_ratio: 0.95,
            scan_ranges: Some(128),
            bytes_read: Some(1024 * 1024 * 100),
            io_time_ms: None,
            cache_hit_rate: None,
            predicates: Some("order_date > '2024-01-01'".to_string()),
            partitions_scanned: Some("10/100".to_string()),
            full_table_path: Some("default_catalog.db.orders".to_string()),
        }];

        let profile_data = ProfileDataForLLM {
            operators: vec![],
            time_distribution: None,
            scan_details,
            join_details: vec![],
            agg_details: vec![],
            exchange_details: vec![],
        };

        let request = RootCauseAnalysisRequest {
            query_summary: QuerySummaryForLLM {
                sql_statement: "SELECT * FROM orders".to_string(),
                query_type: "SELECT".to_string(),
                query_complexity: Some("Simple".to_string()),
                total_time_seconds: 5.0,
                scan_bytes: 100 * 1024 * 1024,
                output_rows: 50000,
                be_count: 3,
                has_spill: false,
                spill_bytes: None,
                session_variables: HashMap::new(),
            },
            profile_data: Some(profile_data),
            execution_plan: ExecutionPlanForLLM {
                dag_description: "SCAN -> AGG".to_string(),
                hotspot_nodes: vec![],
            },
            rule_diagnostics: vec![],
            key_metrics: KeyMetricsForLLM::default(),
            user_question: None,
        };

        let prompt = build_system_prompt(&request);

        // Verify internal table guidance is included
        assert!(prompt.contains("StarRocks 内表"), "Should mention internal tables");
        assert!(prompt.contains("ANALYZE TABLE"), "Should suggest ANALYZE for internal tables");
        assert!(prompt.contains("分桶键"), "Should mention bucket key optimization");

        // Verify critical thinking section
        assert!(prompt.contains("批判性思维"), "Should include critical thinking section");
        assert!(prompt.contains("自我批评"), "Should mention self-criticism");

        // Verify parameter validation
        assert!(prompt.contains("禁止使用的参数"), "Should list forbidden parameters");
        assert!(prompt.contains("enable_short_key_index"), "Should blacklist fake params");

        println!("✅ Internal table prompt test passed!");
        println!("Prompt length: {} chars", prompt.len());
    }

    /// Test dynamic prompt generation with Iceberg external tables
    #[test]
    fn test_prompt_with_iceberg_tables() {
        let scan_details = vec![ScanDetailForLLM {
            plan_node_id: 1,
            table_name: "iceberg_catalog.db.events".to_string(),
            scan_type: "CONNECTOR_SCAN".to_string(),
            table_type: "external".to_string(),
            connector_type: Some("iceberg".to_string()),
            rows_read: 5000000,
            rows_returned: 100000,
            filter_ratio: 0.98,
            scan_ranges: Some(500),
            bytes_read: Some(1024 * 1024 * 1024),
            io_time_ms: Some(5000.0),
            cache_hit_rate: Some(30.0),
            predicates: None,
            partitions_scanned: None,
            full_table_path: Some("iceberg_catalog.db.events".to_string()),
        }];

        let profile_data = ProfileDataForLLM {
            operators: vec![],
            time_distribution: None,
            scan_details,
            join_details: vec![],
            agg_details: vec![],
            exchange_details: vec![],
        };

        let request = RootCauseAnalysisRequest {
            query_summary: QuerySummaryForLLM {
                sql_statement: "SELECT * FROM events".to_string(),
                query_type: "SELECT".to_string(),
                query_complexity: Some("Simple".to_string()),
                total_time_seconds: 30.0,
                scan_bytes: 1024 * 1024 * 1024,
                output_rows: 100000,
                be_count: 3,
                has_spill: false,
                spill_bytes: None,
                session_variables: HashMap::new(),
            },
            profile_data: Some(profile_data),
            execution_plan: ExecutionPlanForLLM {
                dag_description: "CONNECTOR_SCAN -> AGG".to_string(),
                hotspot_nodes: vec![],
            },
            rule_diagnostics: vec![],
            key_metrics: KeyMetricsForLLM::default(),
            user_question: None,
        };

        let prompt = build_system_prompt(&request);

        // Verify Iceberg-specific guidance
        assert!(prompt.contains("Iceberg 外表"), "Should mention Iceberg tables");
        assert!(prompt.contains("rewrite_data_files"), "Should suggest Iceberg file compaction");
        assert!(prompt.contains("DataCache"), "Should suggest DataCache for external tables");
        assert!(
            prompt.contains("不能用 ALTER TABLE 改分桶"),
            "Should warn about external table limitations"
        );

        println!("✅ Iceberg table prompt test passed!");
    }

    /// Test prompt with session variables to avoid redundant suggestions
    #[test]
    fn test_prompt_with_existing_session_vars() {
        let mut session_vars = HashMap::new();
        session_vars.insert("enable_scan_datacache".to_string(), "true".to_string());
        session_vars.insert("enable_spill".to_string(), "true".to_string());
        session_vars.insert("parallel_fragment_exec_instance_num".to_string(), "16".to_string());

        let request = RootCauseAnalysisRequest {
            query_summary: QuerySummaryForLLM {
                sql_statement: "SELECT * FROM t".to_string(),
                query_type: "SELECT".to_string(),
                query_complexity: Some("Simple".to_string()),
                total_time_seconds: 10.0,
                scan_bytes: 0,
                output_rows: 0,
                be_count: 3,
                has_spill: false,
                spill_bytes: None,
                session_variables: session_vars,
            },
            profile_data: None,
            execution_plan: ExecutionPlanForLLM {
                dag_description: "SCAN".to_string(),
                hotspot_nodes: vec![],
            },
            rule_diagnostics: vec![],
            key_metrics: KeyMetricsForLLM::default(),
            user_question: None,
        };

        let prompt = build_system_prompt(&request);

        // Verify session variables are included
        assert!(prompt.contains("enable_scan_datacache"), "Should show current datacache setting");
        assert!(prompt.contains("enable_spill"), "Should show current spill setting");
        assert!(prompt.contains("不要重复建议"), "Should warn about duplicate suggestions");

        println!("✅ Session variables prompt test passed!");
    }

    /// Test prompt with rule engine diagnostics
    #[test]
    fn test_prompt_with_diagnostics() {
        let diagnostics = vec![
            DiagnosticForLLM {
                rule_id: "SCAN_HIGH_FILTER_RATIO".to_string(),
                severity: "warning".to_string(),
                operator: "OLAP_SCAN".to_string(),
                plan_node_id: Some(1),
                message: "Filter ratio > 80%, consider adding partition".to_string(),
                evidence: HashMap::new(),
                threshold_info: None,
            },
            DiagnosticForLLM {
                rule_id: "JOIN_SKEW".to_string(),
                severity: "critical".to_string(),
                operator: "HASH_JOIN".to_string(),
                plan_node_id: Some(5),
                message: "Data skew detected in join".to_string(),
                evidence: HashMap::new(),
                threshold_info: None,
            },
        ];

        let request = RootCauseAnalysisRequest {
            query_summary: QuerySummaryForLLM {
                sql_statement: "SELECT * FROM t".to_string(),
                query_type: "SELECT".to_string(),
                query_complexity: Some("Simple".to_string()),
                total_time_seconds: 10.0,
                scan_bytes: 0,
                output_rows: 0,
                be_count: 3,
                has_spill: false,
                spill_bytes: None,
                session_variables: HashMap::new(),
            },
            profile_data: None,
            execution_plan: ExecutionPlanForLLM {
                dag_description: "SCAN -> JOIN".to_string(),
                hotspot_nodes: vec![],
            },
            rule_diagnostics: diagnostics,
            key_metrics: KeyMetricsForLLM::default(),
            user_question: None,
        };

        let prompt = build_system_prompt(&request);

        // Verify diagnostics are included
        assert!(prompt.contains("SCAN_HIGH_FILTER_RATIO"), "Should include rule IDs");
        assert!(prompt.contains("JOIN_SKEW"), "Should include all diagnostics");
        assert!(prompt.contains("规则引擎已识别的问题"), "Should have diagnostics section");
        assert!(prompt.contains("隐式问题"), "Should guide to find hidden issues");

        println!("✅ Diagnostics prompt test passed!");
    }

    /// Test prompt output includes JSON format specification
    #[test]
    fn test_prompt_includes_json_format() {
        let request = RootCauseAnalysisRequest {
            query_summary: QuerySummaryForLLM {
                sql_statement: "SELECT 1".to_string(),
                query_type: "SELECT".to_string(),
                query_complexity: Some("Simple".to_string()),
                total_time_seconds: 0.1,
                scan_bytes: 0,
                output_rows: 1,
                be_count: 1,
                has_spill: false,
                spill_bytes: None,
                session_variables: HashMap::new(),
            },
            profile_data: None,
            execution_plan: ExecutionPlanForLLM {
                dag_description: "".to_string(),
                hotspot_nodes: vec![],
            },
            rule_diagnostics: vec![],
            key_metrics: KeyMetricsForLLM::default(),
            user_question: None,
        };

        let prompt = build_system_prompt(&request);

        // Verify JSON format is specified
        assert!(prompt.contains("root_causes"), "Should specify root_causes field");
        assert!(prompt.contains("recommendations"), "Should specify recommendations field");
        assert!(prompt.contains("sql_example"), "Should require sql_example");
        assert!(prompt.contains("JSON"), "Should mention JSON format");

        println!("✅ JSON format prompt test passed!");
    }
}

// ============================================================================
// Table Type Detection Tests
// ============================================================================

mod table_type_tests {
    use super::{determine_connector_type, determine_table_type};
    use std::collections::HashMap;

    #[test]
    fn test_determine_table_type() {
        // Internal tables
        assert_eq!(determine_table_type("default_catalog.db.table"), "internal");
        assert_eq!(determine_table_type("db.table"), "internal");
        assert_eq!(determine_table_type("table"), "internal");

        // External tables
        assert_eq!(determine_table_type("hive_catalog.db.table"), "external");
        assert_eq!(determine_table_type("iceberg_cat.db.table"), "external");
        assert_eq!(determine_table_type("my_lake.schema.table"), "external");

        println!("✅ Table type detection tests passed!");
    }

    #[test]
    fn test_determine_connector_type() {
        // Iceberg detection
        let mut metrics = HashMap::new();
        metrics.insert("IcebergV2FormatTimer".to_string(), "100ms".to_string());
        assert_eq!(determine_connector_type(&metrics), "iceberg");

        // Hive/ORC detection
        let mut metrics = HashMap::new();
        metrics.insert("ORC".to_string(), "".to_string());
        metrics.insert("TotalStripeSize".to_string(), "1GB".to_string());
        assert_eq!(determine_connector_type(&metrics), "hive");

        // Hudi detection
        let mut metrics = HashMap::new();
        metrics.insert("HudiScanTimer".to_string(), "50ms".to_string());
        assert_eq!(determine_connector_type(&metrics), "hudi");

        // JDBC detection
        let mut metrics = HashMap::new();
        metrics.insert("JDBCReadRows".to_string(), "1000".to_string());
        assert_eq!(determine_connector_type(&metrics), "jdbc");

        println!("✅ Connector type detection tests passed!");
    }
}

// ============================================================================
// SQL Diagnosis Tests
// ============================================================================

mod sql_diag_tests {
    use super::*;
    use crate::services::llm::scenarios::sql_diag::{SqlDiagReq, SqlDiagResp};

    /// Test SQL diagnosis with real LLM
    /// Run with: cargo test sql_diag_tests::test_sql_diagnosis_llm --lib -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_sql_diagnosis_llm() {
        let sep = "=".repeat(80);
        println!("\n{}", sep);
        println!("🧪 SQL Diagnosis LLM Integration Test");
        println!("{}\n", sep);

        // Connect to real database
        let db_paths = vec![
            "data/starrocks-admin.db",
            "starrocks_admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/data/starrocks-admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/starrocks_admin.db",
        ];
        let db_path = db_paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .expect("Database not found. Run backend first to initialize.");
        println!("📁 Using database: {}", db_path);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", db_path))
            .await
            .expect("Failed to connect to database");

        let llm_service = LLMServiceImpl::new(pool, true, 24);

        if !llm_service.is_available() {
            println!("⚠️  No active LLM provider found.");
            return;
        }

        // Build test request with EXPLAIN
        let sql = r#"SELECT o.order_id, o.customer_id, c.name, o.amount
FROM orders o
JOIN customers c ON o.customer_id = c.id
WHERE o.order_date >= '2024-01-01'
ORDER BY o.amount DESC
LIMIT 100"#;

        let explain = r#"PLAN FRAGMENT 0
  OUTPUT EXPRS: 1: order_id | 2: customer_id | 5: name | 4: amount
  PARTITION: UNPARTITIONED
  RESULT SINK
    EXCHANGE ID: 04
    
PLAN FRAGMENT 1
  OUTPUT EXPRS:
  PARTITION: HASH_PARTITIONED: 2: customer_id
  STREAM DATA SINK
    EXCHANGE ID: 04
    UNPARTITIONED
    
  3:HASH JOIN
     |  join op: INNER JOIN (BROADCAST)
     |  colocate: false
     |  equal join conjunct: 2: customer_id = 6: id
     |  cardinality=1000000
     |
     |----2:EXCHANGE
     |       distribution type: BROADCAST
     |
     1:OlapScanNode
        TABLE: orders
        PREAGGREGATION: ON
        partitions=30/30
        rollup: orders
        tabletRatio=480/480
        cardinality=10000000
        avgRowSize=32.0
        
PLAN FRAGMENT 2
  OUTPUT EXPRS:
  PARTITION: RANDOM
  STREAM DATA SINK
    EXCHANGE ID: 02
    BROADCAST
    
  0:OlapScanNode
     TABLE: customers
     PREAGGREGATION: ON
     partitions=1/1
     rollup: customers
     tabletRatio=16/16
     cardinality=50000"#;

        let schema = serde_json::json!({
            "orders": {
                "rows": 10000000,
                "partition": {"type": "RANGE", "key": "order_date"},
                "dist": {"key": "order_id", "buckets": 16}
            },
            "customers": {
                "rows": 50000,
                "dist": {"key": "id", "buckets": 16}
            }
        });

        let req = SqlDiagReq {
            sql: sql.to_string(),
            explain: Some(explain.to_string()),
            schema: Some(schema),
            vars: Some(serde_json::json!({"pipeline_dop": "0", "enable_spill": "true"})),
        };

        println!("📤 Request:");
        println!("{}", serde_json::to_string_pretty(&req).unwrap());

        println!("\n🤖 Calling LLM...\n");
        let start = std::time::Instant::now();
        let result = llm_service.analyze::<SqlDiagReq, SqlDiagResp>(&req, "test_diag", None, false).await;
        let elapsed = start.elapsed();

        match result {
            Ok(r) => {
                println!("⏱️  LLM call took: {:?} (from_cache: {})\n", elapsed, r.from_cache);
                println!("📥 Response:");
                println!("{}", serde_json::to_string_pretty(&r.response).unwrap());

                // Validate response
                println!("\n📊 Validation:");
                println!("   - SQL changed: {}", r.response.changed);
                println!("   - Perf issues: {}", r.response.perf_issues.len());
                println!("   - Confidence: {:.0}%", r.response.confidence * 100.0);
                println!("   - Summary: {}", r.response.summary);

                assert!(r.response.confidence > 0.0, "Confidence should be > 0");
                assert!(!r.response.summary.is_empty(), "Summary should not be empty");
            }
            Err(e) => {
                println!("❌ LLM call failed: {}", e);
                panic!("LLM call failed: {}", e);
            }
        }
    }

    /// Test complex SQL diagnosis (user retention analysis)
    /// Run with: cargo test sql_diag_tests::test_complex_sql_diagnosis --lib -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_complex_sql_diagnosis() {
        let sep = "=".repeat(80);
        println!("\n{}", sep);
        println!("🧪 Complex SQL Diagnosis - User Retention Analysis");
        println!("{}\n", sep);

        // Connect to real database
        let db_paths = vec![
            "data/starrocks-admin.db",
            "starrocks_admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/data/starrocks-admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/starrocks_admin.db",
        ];
        let db_path = db_paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .expect("Database not found.");
        println!("📁 Using database: {}", db_path);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", db_path))
            .await
            .expect("Failed to connect to database");

        let llm_service = LLMServiceImpl::new(pool, true, 24);

        if !llm_service.is_available() {
            println!("⚠️  No active LLM provider found.");
            return;
        }

        // Complex user retention analysis SQL
        let sql = r#"WITH app_usage_with_name AS (
    SELECT 
        l.statis_id,
        SUM(l.start_duration) / 60.0 AS start_dr_min,
        COALESCE(d.app_name, '未知APP') AS app_name
    FROM cpc_dw_common.dws_s01_commom_os_app_launched_inc_d l
    INNER JOIN cpc_tmp.ads_user_retention_label_202510 urf ON l.statis_id = urf.statis_id
    LEFT JOIN cpc_dw_common.dim_s06_common_pack_name_category_all_d d ON l.pack_name = d.pack_name
        AND d.dayno = '20251130'
    WHERE l.dayno BETWEEN '20251101' AND '20251130'
        AND l.start_duration > 0
        AND l.statis_id IS NOT NULL
    GROUP BY l.statis_id, d.app_name
),
app_ranking AS (
    SELECT 
        app_name,
        SUM(CASE WHEN urf.is_retained = 1 THEN start_dr_min ELSE 0 END) AS retained_total_dr
    FROM app_usage_with_name a
    INNER JOIN cpc_tmp.ads_user_retention_label_202510 urf ON a.statis_id = urf.statis_id
    GROUP BY app_name
),
app_mapping AS (
    SELECT 
        app_name,
        CASE WHEN ROW_NUMBER() OVER (ORDER BY retained_total_dr DESC) <= 30 THEN app_name
             ELSE '其他'
        END AS adj_app_name
    FROM app_ranking
)
SELECT 
    user_type_cn,
    app_name,
    retained_total_dr_min,
    retained_total_uv,
    churned_total_dr_min,
    churned_total_uv
FROM retention_comparison
WHERE (retained_total_dr_min > 0 OR churned_total_dr_min > 0)
ORDER BY (retained_total_dr_min + churned_total_dr_min) DESC
LIMIT 50000"#;

        let req = SqlDiagReq {
            sql: sql.to_string(),
            explain: None,  // No EXPLAIN for complex analysis
            schema: None,   
            vars: None,     
        };

        println!("📤 Complex SQL (truncated):");
        let sql_preview = if sql.len() > 500 { 
            format!("{}...", &sql[..500]) 
        } else { 
            sql.to_string() 
        };
        println!("{}", sql_preview);

        println!("\n🤖 Calling LLM for complex SQL analysis...\n");
        let start = std::time::Instant::now();
        let result = llm_service.analyze::<SqlDiagReq, SqlDiagResp>(&req, "test_complex", None, true).await;
        let elapsed = start.elapsed();

        match result {
            Ok(r) => {
                println!("⏱️  LLM call took: {:?}\n", elapsed);
                println!("📥 Response:");
                println!("{}", serde_json::to_string_pretty(&r.response).unwrap());

                println!("\n📊 Analysis Results:");
                println!("   - SQL changed: {}", r.response.changed);
                println!("   - Confidence: {:.0}%", r.response.confidence * 100.0);
                println!("   - Performance issues found: {}", r.response.perf_issues.len());
                
                for (i, issue) in r.response.perf_issues.iter().enumerate() {
                    println!("   {}. [{}] {} - {}", i+1, issue.severity, issue.r#type, issue.desc);
                    if let Some(fix) = &issue.fix {
                        println!("      💡 Fix: {}", fix);
                    }
                }
                
                println!("   - Summary: {}", r.response.summary);

                // Complex SQL should get meaningful analysis
                assert!(r.response.confidence > 0.0, "Complex SQL should get some confidence");
                assert!(!r.response.summary.is_empty(), "Summary should not be empty");
            }
            Err(e) => {
                println!("❌ LLM call failed: {}", e);
                panic!("LLM call failed: {}", e);
            }
        }
    }

    /// Test SQL diagnosis without EXPLAIN (static analysis only)
    /// Run with: cargo test sql_diag_tests::test_sql_diagnosis_no_explain --lib -- --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_sql_diagnosis_no_explain() {
        let sep = "=".repeat(80);
        println!("\n{}", sep);
        println!("🧪 SQL Diagnosis WITHOUT EXPLAIN (Static Analysis)");
        println!("{}\n", sep);

        // Connect to real database
        let db_paths = vec![
            "data/starrocks-admin.db",
            "starrocks_admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/data/starrocks-admin.db",
            "/home/oppo/Documents/starrocks-admin/backend/starrocks_admin.db",
        ];
        let db_path = db_paths
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .expect("Database not found.");
        println!("📁 Using database: {}", db_path);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", db_path))
            .await
            .expect("Failed to connect to database");

        let llm_service = LLMServiceImpl::new(pool, true, 24);

        if !llm_service.is_available() {
            println!("⚠️  No active LLM provider found.");
            return;
        }

        // Build test request WITHOUT EXPLAIN (simulating frontend scenario)
        let sql = r#"SELECT * FROM orders o
JOIN customers c ON o.customer_id = c.id
ORDER BY o.created_at DESC"#;

        let req = SqlDiagReq {
            sql: sql.to_string(),
            explain: None,  // No EXPLAIN!
            schema: None,   // No schema!
            vars: None,     // No vars!
        };

        println!("📤 Request (NO EXPLAIN):");
        println!("{}", serde_json::to_string_pretty(&req).unwrap());

        println!("\n🤖 Calling LLM for static analysis...\n");
        let start = std::time::Instant::now();
        let result = llm_service.analyze::<SqlDiagReq, SqlDiagResp>(&req, "test_no_explain", None, true).await;
        let elapsed = start.elapsed();

        match result {
            Ok(r) => {
                println!("⏱️  LLM call took: {:?}\n", elapsed);
                println!("📥 Response:");
                println!("{}", serde_json::to_string_pretty(&r.response).unwrap());

                println!("\n📊 Validation:");
                println!("   - Confidence: {:.0}%", r.response.confidence * 100.0);
                println!("   - Perf issues: {}", r.response.perf_issues.len());
                println!("   - Summary: {}", r.response.summary);

                // Even without EXPLAIN, we should get some analysis
                assert!(r.response.confidence > 0.0, "Confidence should be > 0 even without EXPLAIN");
                assert!(!r.response.summary.is_empty(), "Summary should not be empty");
            }
            Err(e) => {
                println!("❌ LLM call failed: {}", e);
                panic!("LLM call failed: {}", e);
            }
        }
    }

    /// Test SqlDiagResp deserialization with various JSON formats
    #[test]
    fn test_sql_diag_resp_deserialization() {
        // Test 1: Full response
        let json = r#"{
            "sql": "SELECT * FROM orders WHERE order_date >= '2024-01-01'",
            "changed": true,
            "perf_issues": [
                {"type": "full_scan", "severity": "high", "desc": "Full table scan", "fix": "Add partition filter"}
            ],
            "explain_analysis": {
                "scan_type": "full_scan",
                "join_strategy": "broadcast",
                "estimated_rows": 10000000,
                "estimated_cost": "high"
            },
            "summary": "Found 1 high severity issue",
            "confidence": 0.85
        }"#;
        let resp: SqlDiagResp = serde_json::from_str(json).expect("Failed to parse full response");
        assert!(resp.changed);
        assert_eq!(resp.perf_issues.len(), 1);
        assert_eq!(resp.confidence, 0.85);
        println!("✅ Full response parsed correctly");

        // Test 2: Minimal response (all defaults)
        let json = r#"{}"#;
        let resp: SqlDiagResp = serde_json::from_str(json).expect("Failed to parse empty response");
        assert!(!resp.changed);
        assert!(resp.perf_issues.is_empty());
        assert_eq!(resp.confidence, 0.0);
        println!("✅ Empty response parsed with defaults");

        // Test 3: Response with missing optional fields
        let json = r#"{
            "sql": "SELECT 1",
            "changed": false,
            "summary": "No issues found",
            "confidence": 0.9
        }"#;
        let resp: SqlDiagResp = serde_json::from_str(json).expect("Failed to parse partial response");
        assert!(!resp.changed);
        assert!(resp.perf_issues.is_empty());
        assert!(resp.explain_analysis.is_none());
        assert_eq!(resp.confidence, 0.9);
        println!("✅ Partial response parsed correctly");

        // Test 4: Response with perf_issues but no fix
        let json = r#"{
            "sql": "SELECT * FROM t",
            "changed": false,
            "perf_issues": [
                {"type": "select_star", "severity": "low", "desc": "Using SELECT *"}
            ],
            "summary": "Minor issue found",
            "confidence": 0.7
        }"#;
        let resp: SqlDiagResp = serde_json::from_str(json).expect("Failed to parse response without fix");
        assert_eq!(resp.perf_issues.len(), 1);
        assert!(resp.perf_issues[0].fix.is_none());
        println!("✅ Response without fix parsed correctly");

        // Test 5: Response with "unknown" estimated_rows (string instead of number)
        let json = r#"{
            "sql": "SELECT * FROM t",
            "changed": false,
            "explain_analysis": {
                "scan_type": "unknown",
                "join_strategy": "unknown", 
                "estimated_rows": "unknown",
                "estimated_cost": "unknown"
            },
            "summary": "Analysis with unknown values",
            "confidence": 0.5
        }"#;
        let resp: SqlDiagResp = serde_json::from_str(json).expect("Failed to parse response with unknown values");
        assert!(resp.explain_analysis.is_some());
        let analysis = resp.explain_analysis.unwrap();
        assert_eq!(analysis.scan_type, Some("unknown".to_string()));
        assert!(analysis.estimated_rows.is_none()); // "unknown" string becomes None
        println!("✅ Response with 'unknown' string values parsed correctly");
    }
}
