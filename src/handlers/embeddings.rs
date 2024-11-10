use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use crate::{
    models::embeddings::{EmbeddingsRequest, EmbeddingsResponse},
    state::AppState,
};

pub async fn embeddings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmbeddingsRequest>,
) -> Result<Json<EmbeddingsResponse>, StatusCode> {
    for model in state.config.models.iter() {
        if let Some(model) = state.model_registry.get(&model.key) {
            let response = model.embeddings(state.clone(), payload.clone()).await?;
            return Ok(Json(response));
        }
    }

    Err(StatusCode::SERVICE_UNAVAILABLE)
}
