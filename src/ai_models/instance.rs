use crate::config::models::ModelConfig;
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;
use axum::http::StatusCode;
use std::sync::Arc;

pub struct ModelInstance {
    pub name: String,
    pub model_type: String,
    pub provider: Arc<dyn Provider>,
    pub config: ModelConfig,
}

impl ModelInstance {
    pub async fn chat_completions(
        &self,
        mut payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        payload.model = self.model_type.clone();
        self.provider.chat_completions(payload, &self.config).await
    }

    pub async fn completions(
        &self,
        mut payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode> {
        payload.model = self.model_type.clone();

        self.provider.completions(payload, &self.config).await
    }

    pub async fn embeddings(
        &self,
        mut payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        payload.model = self.model_type.clone();
        self.provider.embeddings(payload, &self.config).await
    }
}
