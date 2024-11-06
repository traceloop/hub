use std::sync::Arc;
use axum::http::StatusCode;
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;
use crate::state::AppState;

pub struct ModelInstance {
    pub name: String,
    pub model_type: String,
    pub provider: Arc<dyn Provider>,
}

impl ModelInstance {
    pub async fn chat_completions(
        &self,
        state: Arc<AppState>,
        mut payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        payload.model = self.model_type.clone();
        self.provider.chat_completions(state, payload).await
    }

    pub async fn completions(
        &self,
        state: Arc<AppState>,
        mut payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode> {
        payload.model = self.model_type.clone();
        
        self.provider.completions(state, payload).await
    }

    pub async fn embeddings(
        &self,
        state: Arc<AppState>,
        mut payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        payload.model = self.model_type.clone();
        self.provider.embeddings(state, payload).await
    }
}
