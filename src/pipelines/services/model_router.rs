use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;

use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::state::AppState;

pub struct ModelRouterService {
    state: Arc<AppState>,
    models: Vec<String>,
}

impl ModelRouterService {
    pub fn new(state: Arc<AppState>, models: Vec<String>) -> Self {
        Self { state, models }
    }
}

impl Service<ChatCompletionRequest> for ModelRouterService {
    type Response = ChatCompletionResponse;
    type Error = axum::http::StatusCode;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: ChatCompletionRequest) -> Self::Future {
        let state = self.state.clone();
        let models = self.models.clone();

        Box::pin(async move {
            // Try each model in order until we find one that works
            for model_name in models {
                if let Some(model) = state.model_registry.get(&model_name) {
                    match model.chat_completions(state.clone(), request.clone()).await {
                        Ok(response) => return Ok(response),
                        Err(e) => return Err(e),
                    }
                }
            }

            // If no models are available, return an error
            Err(axum::http::StatusCode::SERVICE_UNAVAILABLE)
        })
    }
}
