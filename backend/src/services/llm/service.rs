//! LLM Service Trait and Implementation
//!
//! Defines the generic LLM service interface and its implementation.

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

use super::client::LLMClient;
use super::models::*;
use super::repository::LLMRepository;

// ============================================================================
// LLM Analysis Request/Response Traits
// ============================================================================

/// Trait for LLM analysis requests
/// Implemented by each scenario (RootCause, SqlOptimization, etc.)
pub trait LLMAnalysisRequestTrait: Serialize + Send + Sync {
    /// The scenario type for this request
    fn scenario(&self) -> LLMScenario;

    /// Get the system prompt for this scenario (dynamic, based on request context)
    fn system_prompt(&self) -> String;

    /// Build cache key for deduplication
    fn cache_key(&self) -> String;

    /// Get SQL hash for tracking
    fn sql_hash(&self) -> String;

    /// Get profile hash for tracking
    fn profile_hash(&self) -> String;
}

/// Trait for LLM analysis responses
pub trait LLMAnalysisResponseTrait: DeserializeOwned + Serialize + Send + Sync {
    /// Get summary text for logging
    fn summary(&self) -> &str;

    /// Get confidence score (if applicable)
    fn confidence(&self) -> Option<f64>;
}

// ============================================================================
// LLM Service Trait
// ============================================================================

/// LLM Analysis result with metadata
#[derive(Debug, Clone)]
pub struct LLMAnalysisResult<T> {
    /// The actual response
    pub response: T,
    /// Whether this result was from cache
    pub from_cache: bool,
}

/// LLM Service - the core abstraction for all LLM operations
#[async_trait]
pub trait LLMService: Send + Sync {
    /// Check if LLM service is available
    fn is_available(&self) -> bool;

    /// Get the currently active provider info
    fn active_provider(&self) -> Option<LLMProviderInfo>;

    /// Analyze with LLM, returns structured response with cache metadata
    ///
    /// # Parameters
    /// - `force_refresh`: If true, bypass cache and force LLM API call
    async fn analyze<Req, Resp>(
        &self,
        request: &Req,
        query_id: &str,
        cluster_id: Option<i64>,
        force_refresh: bool,
    ) -> Result<LLMAnalysisResult<Resp>, LLMError>
    where
        Req: LLMAnalysisRequestTrait,
        Resp: LLMAnalysisResponseTrait;

    /// Get all providers
    async fn list_providers(&self) -> Result<Vec<LLMProviderInfo>, LLMError>;

    /// Get provider by ID
    async fn get_provider(&self, id: i64) -> Result<Option<LLMProviderInfo>, LLMError>;

    /// Get active provider
    async fn get_active_provider(&self) -> Result<Option<LLMProviderInfo>, LLMError>;

    /// Create a new provider
    async fn create_provider(&self, req: CreateProviderRequest) -> Result<LLMProvider, LLMError>;

    /// Update a provider
    async fn update_provider(
        &self,
        id: i64,
        req: UpdateProviderRequest,
    ) -> Result<LLMProvider, LLMError>;

    /// Delete a provider
    async fn delete_provider(&self, id: i64) -> Result<(), LLMError>;

    /// Activate a provider
    async fn activate_provider(&self, provider_id: i64) -> Result<(), LLMError>;

    /// Deactivate a provider
    async fn deactivate_provider(&self, provider_id: i64) -> Result<(), LLMError>;

    /// Test connection to a provider
    async fn test_connection(&self, provider_id: i64) -> Result<TestConnectionResponse, LLMError>;
}

// ============================================================================
// LLM Service Implementation
// ============================================================================

/// LLM Service implementation
pub struct LLMServiceImpl {
    repository: LLMRepository,
    client: LLMClient,
    enabled: bool,
    cache_ttl_hours: i64,
}

impl LLMServiceImpl {
    /// Create a new LLM service
    pub fn new(pool: sqlx::SqlitePool, enabled: bool, cache_ttl_hours: i64) -> Self {
        Self {
            repository: LLMRepository::new(pool),
            client: LLMClient::new(),
            enabled,
            cache_ttl_hours,
        }
    }

    /// Create with custom client (for testing)
    pub fn with_client(
        pool: sqlx::SqlitePool,
        client: LLMClient,
        enabled: bool,
        cache_ttl_hours: i64,
    ) -> Self {
        Self { repository: LLMRepository::new(pool), client, enabled, cache_ttl_hours }
    }
}

#[async_trait]
impl LLMService for LLMServiceImpl {
    fn is_available(&self) -> bool {
        self.enabled
    }

    fn active_provider(&self) -> Option<LLMProviderInfo> {
        // This is a sync method, so we can't query DB here
        // The actual provider is fetched in analyze()
        None
    }

    async fn analyze<Req, Resp>(
        &self,
        request: &Req,
        query_id: &str,
        cluster_id: Option<i64>,
        force_refresh: bool,
    ) -> Result<LLMAnalysisResult<Resp>, LLMError>
    where
        Req: LLMAnalysisRequestTrait,
        Resp: LLMAnalysisResponseTrait,
    {
        if !self.enabled {
            return Err(LLMError::Disabled);
        }

        // 1. Get active provider
        let provider = self
            .repository
            .get_active_provider()
            .await?
            .ok_or(LLMError::NoProviderConfigured)?;

        // 2. Check cache (skip if force_refresh is true)
        let cache_key = request.cache_key();
        let sql_hash = request.sql_hash();
        let profile_hash = request.profile_hash();
        tracing::info!(
            "LLM request - cache_key: {}, sql_hash: {}, profile_hash: {}, force_refresh: {}",
            cache_key,
            sql_hash,
            profile_hash,
            force_refresh
        );

        if !force_refresh {
            if let Some(cached) = self.repository.get_cached_response(&cache_key).await? {
                tracing::info!("✅ LLM cache HIT for key: {}", cache_key);
                let response: Resp = serde_json::from_str(&cached).map_err(LLMError::from)?;
                return Ok(LLMAnalysisResult { response, from_cache: true });
            }
        } else {
            tracing::info!("🔄 Force refresh requested, bypassing cache for key: {}", cache_key);
        }

        tracing::info!("❌ LLM cache MISS for key: {}, calling API...", cache_key);

        // 3. Create session
        let session_id = self
            .repository
            .create_session(query_id, provider.id, cluster_id, request.scenario())
            .await?;

        // 4. Save request for debugging
        let request_json = serde_json::to_string(request)?;
        self.repository
            .save_request(&session_id, &request_json, &request.sql_hash(), &request.profile_hash())
            .await?;

        // 5. Update session to processing
        self.repository
            .update_session_status(&session_id, SessionStatus::Processing)
            .await?;

        // 6. Call LLM API
        let start = std::time::Instant::now();
        let result = self
            .client
            .chat_completion::<Req, Resp>(&provider, request)
            .await;
        let latency_ms = start.elapsed().as_millis() as i32;

        match result {
            Ok((response, input_tokens, output_tokens)) => {
                // 7. Save result
                let response_json = serde_json::to_string(&response)?;
                self.repository
                    .save_result(&session_id, &response_json, response.confidence())
                    .await?;

                // 8. Update session to completed
                self.repository
                    .complete_session(
                        &session_id,
                        SessionStatus::Completed,
                        input_tokens,
                        output_tokens,
                        latency_ms,
                        None,
                    )
                    .await?;

                // 9. Cache response
                self.repository
                    .cache_response(
                        &cache_key,
                        request.scenario(),
                        &request.sql_hash(),
                        &response_json,
                        self.cache_ttl_hours,
                    )
                    .await?;

                Ok(LLMAnalysisResult { response, from_cache: false })
            },
            Err(e) => {
                let err_msg = e.to_string();
                self.repository
                    .complete_session(
                        &session_id,
                        SessionStatus::Failed,
                        0,
                        0,
                        latency_ms,
                        Some(err_msg.as_str()),
                    )
                    .await?;
                Err(e)
            },
        }
    }

    async fn list_providers(&self) -> Result<Vec<LLMProviderInfo>, LLMError> {
        let providers = self.repository.list_providers().await?;
        Ok(providers.iter().map(LLMProviderInfo::from).collect())
    }

    async fn get_provider(&self, id: i64) -> Result<Option<LLMProviderInfo>, LLMError> {
        let provider = self.repository.get_provider(id).await?;
        Ok(provider.map(|p| LLMProviderInfo::from(&p)))
    }

    async fn get_active_provider(&self) -> Result<Option<LLMProviderInfo>, LLMError> {
        let provider = self.repository.get_active_provider().await?;
        Ok(provider.map(|p| LLMProviderInfo::from(&p)))
    }

    async fn create_provider(&self, req: CreateProviderRequest) -> Result<LLMProvider, LLMError> {
        self.repository.create_provider(req).await
    }

    async fn update_provider(
        &self,
        id: i64,
        req: UpdateProviderRequest,
    ) -> Result<LLMProvider, LLMError> {
        self.repository.update_provider(id, req).await
    }

    async fn delete_provider(&self, id: i64) -> Result<(), LLMError> {
        self.repository.delete_provider(id).await
    }

    async fn activate_provider(&self, provider_id: i64) -> Result<(), LLMError> {
        self.repository.activate_provider(provider_id).await
    }

    async fn deactivate_provider(&self, provider_id: i64) -> Result<(), LLMError> {
        self.repository.deactivate_provider(provider_id).await
    }

    async fn test_connection(&self, provider_id: i64) -> Result<TestConnectionResponse, LLMError> {
        let provider = self
            .repository
            .get_provider(provider_id)
            .await?
            .ok_or_else(|| LLMError::ProviderNotFound(provider_id.to_string()))?;

        let start = std::time::Instant::now();

        // Simple test: send a minimal request to check connectivity
        let test_result = self.client.test_connection(&provider).await;
        let latency_ms = start.elapsed().as_millis() as i64;

        match test_result {
            Ok(_) => Ok(TestConnectionResponse {
                success: true,
                message: "Connection successful".to_string(),
                latency_ms: Some(latency_ms),
            }),
            Err(e) => Ok(TestConnectionResponse {
                success: false,
                message: format!("Connection failed: {}", e),
                latency_ms: Some(latency_ms),
            }),
        }
    }
}

// Note: Arc<T> automatically implements LLMService through async_trait delegation
// when T: LLMService, so we don't need to manually implement it.
