use crate::chat::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::state::AppState;
use std::sync::Arc;

pub trait Provider {
    fn chat_completions(
        state: Arc<AppState>,
        payload: ChatCompletionRequest,
    ) -> impl std::future::Future<Output = ChatCompletionResponse> + Send;
}