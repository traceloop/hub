use std::sync::Arc;
use crate::chat::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::providers::Provider;
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
    ) -> ChatCompletionResponse {
        payload.model = self.model_type.clone();
        self.provider.chat_completions(state, payload).await
    }
}
