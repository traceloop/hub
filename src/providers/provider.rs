use axum::async_trait;
use axum::http::StatusCode;

use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::config::models::Provider as ProviderConfig;
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::state::AppState;
use std::sync::Arc;


#[async_trait]
pub trait Provider: Send + Sync {
    fn new(config: &ProviderConfig) -> Self where Self: Sized;
    fn name(&self) -> String;
    fn r#type(&self) -> String;

    async fn chat_completions(
        &self,
        state: Arc<AppState>,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode>;
    
    async fn completions(
        &self,
        state: Arc<AppState>,
        payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode>;

    async fn embeddings(
        &self,
        state: Arc<AppState>,
        payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode>;
}
