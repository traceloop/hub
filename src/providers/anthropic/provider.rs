use async_trait::async_trait;
use axum::http::StatusCode;
use reqwest::Client;

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
                Ok(ChatCompletionResponse::NonStream(anthropic_response.into()))
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
