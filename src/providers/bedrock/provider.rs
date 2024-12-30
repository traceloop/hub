use axum::async_trait;
use axum::http::StatusCode;
use reqwest::Client;

use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;


pub struct BedrockProvider {
    config: ProviderConfig,
    // TODO: remove what is below
    // I would not use this but using azure as template
    // I will be use the aws sdk directly this is not needed
    http_client: Client,
}

#[async_trait]
impl Provider for BedrockProvider {
    fn new(config: &ProviderConfig) -> Self
    where
        Self: Sized
    {
        todo!()
    }

    fn key(&self) -> String {
        todo!()
    }

    fn r#type(&self) -> String {
        todo!()
    }

    async fn chat_completions(&self, payload: ChatCompletionRequest, model_config: &ModelConfig) -> Result<ChatCompletionResponse, StatusCode> {
        todo!()
    }

    async fn completions(&self, payload: CompletionRequest, model_config: &ModelConfig) -> Result<CompletionResponse, StatusCode> {
        todo!()
    }

    async fn embeddings(&self, payload: EmbeddingsRequest, model_config: &ModelConfig) -> Result<EmbeddingsResponse, StatusCode> {
        todo!()
    }
}