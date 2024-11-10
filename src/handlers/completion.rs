use crate::{
    models::completion::{CompletionRequest, CompletionResponse},
    state::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

pub async fn completions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CompletionRequest>,
) -> Result<Json<CompletionResponse>, StatusCode> {
    for model in state.config.models.iter() {
        if let Some(model) = state.model_registry.get(&model.key) {
            let response = model.completions(state.clone(), payload.clone()).await?;
            return Ok(Json(response));
        }
    }

    Err(StatusCode::SERVICE_UNAVAILABLE)
}
