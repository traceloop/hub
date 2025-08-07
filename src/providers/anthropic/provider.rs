use async_trait::async_trait;
use axum::http::StatusCode;
use reqwest::Client;
use tracing::info;

use super::models::{AnthropicChatCompletionRequest, AnthropicChatCompletionResponse};
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;

pub struct AnthropicProvider {
    api_key: String,
    config: ProviderConfig,
    http_client: Client,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            config: config.clone(),
            http_client: Client::new(),
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "anthropic".to_string()
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        // Validate reasoning config if present
        if let Some(reasoning) = &payload.reasoning {
            if let Err(e) = reasoning.validate() {
                eprintln!("Invalid reasoning config: {}", e);
                return Err(StatusCode::BAD_REQUEST);
            }

            if let Some(max_tokens) = reasoning.max_tokens {
                info!(
                    "✅ Anthropic reasoning enabled with max_tokens: {}",
                    max_tokens
                );
            } else if let Some(thinking_prompt) = reasoning.to_thinking_prompt() {
                info!(
                    "✅ Anthropic reasoning enabled with effort level: {:?} -> prompt: \"{}\"",
                    reasoning.effort,
                    thinking_prompt.chars().take(50).collect::<String>() + "..."
                );
            } else {
                tracing::debug!(
                    "ℹ️ Anthropic reasoning config present but no valid parameters (effort: {:?}, max_tokens: {:?})",
                    reasoning.effort,
                    reasoning.max_tokens
                );
            }
        }

        let exclude_reasoning = payload
            .reasoning
            .as_ref()
            .and_then(|r| r.exclude)
            .unwrap_or(false);

        let request = AnthropicChatCompletionRequest::from(payload);
        let response = self
            .http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Anthropic API request error: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            if request.stream.unwrap_or(false) {
                unimplemented!()
            } else {
                let anthropic_response: AnthropicChatCompletionResponse = response
                    .json()
                    .await
                    .expect("Failed to parse Anthropic response");
                Ok(ChatCompletionResponse::NonStream(
                    anthropic_response.into_chat_completion(exclude_reasoning),
                ))
            }
        } else {
            eprintln!(
                "Anthropic API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        _payload: CompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        unimplemented!()
    }

    async fn embeddings(
        &self,
        _payload: EmbeddingsRequest,
        _model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        unimplemented!()
    }
}
