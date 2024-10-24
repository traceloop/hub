use std::sync::Arc;
use tracing::error;

use super::Provider;
use crate::chat::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::state::AppState;

pub struct OpenAIProvider;

impl Provider for OpenAIProvider {
    async fn chat_completions(
        state: Arc<AppState>,
        payload: ChatCompletionRequest,
    ) -> ChatCompletionResponse {
        let response = state
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set")))
            .json(&payload)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let body = resp.json::<ChatCompletionResponse>().await.expect("Failed to parse response");
                body
            }
            Err(e) => {
                error!("Failed to send request: {:?}", e);
                panic!("Failed to send request");
            }
        }
    }
}
