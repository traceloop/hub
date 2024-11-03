use super::Provider;
use crate::chat::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::config::models::Provider as ProviderConfig;
use crate::state::AppState;
use std::sync::Arc;
use axum::async_trait;

pub struct AnthropicProvider {
    api_key: String,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
        }
    }

    async fn chat_completions(
        &self,
        state: Arc<AppState>,
        payload: ChatCompletionRequest,
    ) -> ChatCompletionResponse {
        let response = state
            .http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await;

        match response {
            Ok(resp) => resp.json().await.expect("Failed to parse response"),
            Err(e) => panic!("Failed to send request: {:?}", e),
        }
    }
}
