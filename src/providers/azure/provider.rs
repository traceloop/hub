use async_trait::async_trait;
use axum::http::StatusCode;
use reqwest_streams::JsonStreamResponse;
use serde::{Deserialize, Serialize};

use crate::config::constants::stream_buffer_size_bytes;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::models::streaming::ChatCompletionChunk;
use crate::providers::provider::Provider;
use crate::types::ProviderType;
use reqwest::Client;
use tracing::info;

#[derive(Serialize, Deserialize, Clone)]
struct AzureChatCompletionRequest {
    #[serde(flatten)]
    base: ChatCompletionRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

impl From<ChatCompletionRequest> for AzureChatCompletionRequest {
    fn from(mut base: ChatCompletionRequest) -> Self {
        let reasoning_effort = base
            .reasoning_effort
            .take()
            .or_else(|| base.reasoning.as_ref().and_then(|r| r.to_openai_effort()));

        // Remove reasoning field from base request since Azure uses reasoning_effort
        base.reasoning = None;

        Self {
            base,
            reasoning_effort,
        }
    }
}

pub struct AzureProvider {
    config: ProviderConfig,
    http_client: Client,
}

impl AzureProvider {
    fn endpoint(&self) -> String {
        if let Some(base_url) = self.config.params.get("base_url") {
            base_url.clone()
        } else {
            format!(
                "https://{}.openai.azure.com/openai/deployments",
                self.config.params.get("resource_name").unwrap(),
            )
        }
    }
    fn api_version(&self) -> String {
        self.config.params.get("api_version").unwrap().clone()
    }
}

#[async_trait]
impl Provider for AzureProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            config: config.clone(),
            http_client: Client::new(),
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> ProviderType {
        ProviderType::Azure
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        // Validate legacy `reasoning` only when top-level `reasoning_effort` isn't set,
        // mirroring the construction precedence in AzureChatCompletionRequest::from.
        if payload.reasoning_effort.is_none() {
            if let Some(reasoning) = &payload.reasoning {
                if let Err(e) = reasoning.validate() {
                    tracing::error!("Invalid reasoning config: {}", e);
                    return Err(StatusCode::BAD_REQUEST);
                }

                if let Some(max_tokens) = reasoning.max_tokens {
                    info!(
                        "✅ Azure reasoning with max_tokens: {} (note: Azure uses effort levels, max_tokens ignored)",
                        max_tokens
                    );
                } else if let Some(effort) = reasoning.to_openai_effort() {
                    info!(
                        "✅ Azure reasoning enabled with effort level: \"{}\"",
                        effort
                    );
                } else {
                    tracing::debug!(
                        "ℹ️ Azure reasoning config present but no valid parameters (effort: {:?}, max_tokens: {:?})",
                        reasoning.effort,
                        reasoning.max_tokens
                    );
                }
            }
        }

        let deployment = model_config.params.get("deployment").unwrap();
        let api_version = self.api_version();
        let url = format!(
            "{}/{}/chat/completions?api-version={}",
            self.endpoint(),
            deployment,
            api_version
        );

        // Convert to Azure-specific request format
        let azure_request = AzureChatCompletionRequest::from(payload.clone());

        let response = self
            .http_client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .json(&azure_request)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Azure OpenAI API request error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            if payload.stream.unwrap_or(false) {
                let stream =
                    response.json_array_stream::<ChatCompletionChunk>(stream_buffer_size_bytes());
                Ok(ChatCompletionResponse::Stream(stream))
            } else {
                response
                    .json()
                    .await
                    .map(ChatCompletionResponse::NonStream)
                    .map_err(|e| {
                        eprintln!("Azure OpenAI API response error: {e}");
                        StatusCode::INTERNAL_SERVER_ERROR
                    })
            }
        } else {
            info!(
                "Azure OpenAI API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        payload: CompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        let deployment = model_config.params.get("deployment").unwrap();
        let api_version = self.api_version();
        let url = format!(
            "{}/{}/completions?api-version={}",
            self.endpoint(),
            deployment,
            api_version
        );

        let response = self
            .http_client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Azure OpenAI API request error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            response.json().await.map_err(|e| {
                eprintln!("Azure OpenAI API response error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })
        } else {
            eprintln!(
                "Azure OpenAI API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let deployment = model_config.params.get("deployment").unwrap();
        let api_version = self.api_version();

        let url = format!(
            "{}/{}/embeddings?api-version={}",
            self.endpoint(),
            deployment,
            api_version
        );

        let response = self
            .http_client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Azure OpenAI API request error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            response.json().await.map_err(|e| {
                eprintln!("Azure OpenAI Embeddings API response error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })
        } else {
            eprintln!(
                "Azure OpenAI Embeddings API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

#[cfg(test)]
mod reasoning_effort_precedence_tests {
    use super::*;
    use crate::models::chat::ReasoningConfig;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};

    fn base_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("hi".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: None,
            reasoning_effort: None,
        }
    }

    #[test]
    fn top_level_reasoning_effort_wins_when_both_present() {
        let mut req = base_request();
        req.reasoning_effort = Some("minimal".to_string());
        req.reasoning = Some(ReasoningConfig {
            effort: Some("high".to_string()),
            max_tokens: None,
            exclude: None,
        });

        let converted = AzureChatCompletionRequest::from(req);
        assert_eq!(converted.reasoning_effort, Some("minimal".to_string()));
        assert!(converted.base.reasoning.is_none());
        assert!(converted.base.reasoning_effort.is_none());
    }

    #[test]
    fn falls_back_to_nested_reasoning_when_top_level_absent() {
        let mut req = base_request();
        req.reasoning = Some(ReasoningConfig {
            effort: Some("low".to_string()),
            max_tokens: None,
            exclude: None,
        });

        let converted = AzureChatCompletionRequest::from(req);
        assert_eq!(converted.reasoning_effort, Some("low".to_string()));
        assert!(converted.base.reasoning.is_none());
    }

    #[test]
    fn uses_top_level_when_only_top_level_set() {
        let mut req = base_request();
        req.reasoning_effort = Some("none".to_string());

        let converted = AzureChatCompletionRequest::from(req);
        assert_eq!(converted.reasoning_effort, Some("none".to_string()));
    }

    #[test]
    fn omits_effort_when_neither_set() {
        let converted = AzureChatCompletionRequest::from(base_request());
        assert_eq!(converted.reasoning_effort, None);
    }
}
