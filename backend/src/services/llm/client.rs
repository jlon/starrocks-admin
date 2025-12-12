//! LLM Client - HTTP client for OpenAI-compatible APIs
//!
//! Uses reqwest to call LLM APIs. Compatible with:
//! - OpenAI
//! - Azure OpenAI
//! - DeepSeek
//! - Other OpenAI-compatible APIs

use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::Duration;

use super::models::*;
use super::service::LLMAnalysisRequestTrait;

/// LLM HTTP Client
pub struct LLMClient {
    http_client: Client,
}

impl Default for LLMClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LLMClient {
    pub fn new() -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self { http_client }
    }

    /// Call chat completion API
    pub async fn chat_completion<Req, Resp>(
        &self,
        provider: &LLMProvider,
        request: &Req,
    ) -> Result<(Resp, i32, i32), LLMError>
    where
        Req: LLMAnalysisRequestTrait,
        Resp: DeserializeOwned,
    {
        let api_key = provider
            .api_key_encrypted
            .as_ref()
            .ok_or_else(|| LLMError::ApiError("API key not configured".to_string()))?;

        // Build user prompt from request
        let user_prompt =
            serde_json::to_string_pretty(request).map_err(LLMError::SerializationError)?;

        // Build chat completion request
        let chat_request = ChatCompletionRequest {
            model: provider.model_name.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: request.system_prompt().to_string(),
                },
                ChatMessage { role: "user".to_string(), content: user_prompt },
            ],
            max_tokens: Some(provider.max_tokens as u32),
            temperature: Some(provider.temperature),
            response_format: Some(ResponseFormat { r#type: "json_object".to_string() }),
        };

        // Build URL
        let url = format!("{}/chat/completions", provider.api_base.trim_end_matches('/'));

        tracing::debug!("Calling LLM API: {} with model {}", url, provider.model_name);

        // Make request with timeout
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .timeout(Duration::from_secs(provider.timeout_seconds as u64))
            .json(&chat_request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout(provider.timeout_seconds as u64)
                } else {
                    LLMError::ApiError(e.to_string())
                }
            })?;

        // Check status
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            // Extract retry-after header if available
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(LLMError::RateLimited(retry_after));
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LLMError::ApiError(format!("API error {}: {}", status, error_text)));
        }

        // Parse response
        let chat_response: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| LLMError::ParseError(e.to_string()))?;

        // Extract content
        let content = chat_response
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .ok_or_else(|| LLMError::ParseError("Empty response from LLM".to_string()))?;

        // Parse structured response
        let result: Resp = serde_json::from_str(content).map_err(|e| {
            LLMError::ParseError(format!(
                "Failed to parse LLM response: {}. Content: {}",
                e, content
            ))
        })?;

        // Extract token counts
        let input_tokens = chat_response
            .usage
            .as_ref()
            .map(|u| u.prompt_tokens)
            .unwrap_or(0);
        let output_tokens = chat_response
            .usage
            .as_ref()
            .map(|u| u.completion_tokens)
            .unwrap_or(0);

        Ok((result, input_tokens, output_tokens))
    }

    /// Test connection to provider (simple models list request)
    pub async fn test_connection(&self, provider: &LLMProvider) -> Result<(), LLMError> {
        let api_key = provider
            .api_key_encrypted
            .as_ref()
            .ok_or_else(|| LLMError::ApiError("API key not configured".to_string()))?;

        // Use models endpoint for a lightweight test
        let url = format!("{}/models", provider.api_base.trim_end_matches('/'));

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LLMError::Timeout(10)
                } else if e.is_connect() {
                    LLMError::ApiError(format!("Connection failed: {}", e))
                } else {
                    LLMError::ApiError(e.to_string())
                }
            })?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(LLMError::ApiError("Invalid API key".to_string()));
        }

        if !status.is_success() {
            // Some providers don't have /models endpoint, try a minimal chat request
            return self.test_with_chat(provider).await;
        }

        Ok(())
    }

    /// Fallback test using minimal chat completion
    async fn test_with_chat(&self, provider: &LLMProvider) -> Result<(), LLMError> {
        let api_key = provider
            .api_key_encrypted
            .as_ref()
            .ok_or_else(|| LLMError::ApiError("API key not configured".to_string()))?;

        let url = format!("{}/chat/completions", provider.api_base.trim_end_matches('/'));

        let test_request = ChatCompletionRequest {
            model: provider.model_name.clone(),
            messages: vec![ChatMessage { role: "user".to_string(), content: "Hi".to_string() }],
            max_tokens: Some(1),
            temperature: Some(0.0),
            response_format: None,
        };

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .timeout(Duration::from_secs(15))
            .json(&test_request)
            .send()
            .await
            .map_err(|e| LLMError::ApiError(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(LLMError::ApiError("Invalid API key".to_string()));
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LLMError::ApiError(format!("API error {}: {}", status, error_text)));
        }

        Ok(())
    }
}

// ============================================================================
// OpenAI API Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    r#type: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
}
