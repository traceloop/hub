use async_trait::async_trait;
use axum::http::StatusCode;
use std::borrow::Cow;

use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::types::ProviderType;

#[async_trait]
pub trait Provider: Send + Sync {
    fn new(config: &ProviderConfig) -> Self
    where
        Self: Sized;
    fn key(&self) -> String;
    fn r#type(&self) -> ProviderType;

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode>;

    async fn completions(
        &self,
        payload: CompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode>;

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode>;
}

/// Maps provider type enum to standardized vendor names for OTEL reporting
pub fn get_vendor_name(provider_type: &ProviderType) -> Cow<'static, str> {
    match provider_type {
        ProviderType::OpenAI => Cow::Borrowed("openai"),
        ProviderType::Azure => Cow::Borrowed("Azure"),
        ProviderType::Anthropic => Cow::Borrowed("Anthropic"),
        ProviderType::Bedrock => Cow::Borrowed("AWS"),
        ProviderType::VertexAI => Cow::Borrowed("Google"),
    }
}
