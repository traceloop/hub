use crate::providers::Provider;
use crate::state::AppState;
use crate::{chat::models::ChatCompletionRequest, models::ModelProvider, utils::extract_provider};
use axum::http::StatusCode;
use axum::{extract::State, Json};
use std::sync::Arc;
use tracing::error;

use super::models::ChatCompletionResponse;

pub async fn completions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, StatusCode> {
    let provider = extract_provider(&headers);
    let response = match provider {
        ModelProvider::OpenAI => {
            crate::providers::OpenAIProvider::chat_completions(state, payload).await
        }
        ModelProvider::Anthropic => {
            crate::providers::AnthropicProvider::chat_completions(state, payload).await
        }
        _ => {
            error!("Unknown provider");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    Ok(Json(response))
}
