use axum::async_trait;

use crate::chat::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::config::models::Provider as ProviderConfig;
use crate::state::AppState;
use std::sync::Arc;

#[async_trait]
pub trait Provider: Send + Sync {
    fn new(config: &ProviderConfig) -> Self where Self: Sized;
    
    async fn chat_completions(
        &self,
        state: Arc<AppState>,
        payload: ChatCompletionRequest,
    ) -> ChatCompletionResponse;
}