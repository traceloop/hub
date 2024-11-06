use axum::async_trait;
use axum::http::StatusCode;

use crate::config::models::Provider as ProviderConfig;
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::state::AppState;
use std::sync::Arc;

use super::provider::Provider;

pub struct OpenAIProvider {
    config: ProviderConfig,
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    fn name(&self) -> String {
        self.config.name.clone()
    }

    fn r#type(&self) -> String {
        "openai".to_string()
    }

    async fn chat_completions(
        &self,
        state: Arc<AppState>,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let response = state
            .http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let status = response.status();
        if status.is_success() {
            response
                .json()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        state: Arc<AppState>,
        payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode> {
        let response = state
            .http_client
            .post("https://api.openai.com/v1/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let status = response.status();
        if status.is_success() {
            response
                .json()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn embeddings(
        &self,
        state: Arc<AppState>,
        payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let response = state
            .http_client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let status = response.status();
        if status.is_success() {
            response
                .json()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}
