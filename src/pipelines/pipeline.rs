use std::sync::Arc;

use crate::config::models::PipelineType;
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::{
    ai_models::registry::ModelRegistry,
    config::models::{Pipeline, PluginConfig},
    models::chat::{ChatCompletionRequest, ChatCompletionResponse},
};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use tower_http::trace::TraceLayer;

pub fn create_pipeline(pipeline: &Pipeline, model_registry: &ModelRegistry) -> Router {
    let mut router = Router::new();

    for plugin in pipeline.plugins.clone() {
        router = match plugin {
            PluginConfig::Tracing { .. } => router.layer(TraceLayer::new_for_http()),
            PluginConfig::ModelRouter { models } => match pipeline.r#type {
                PipelineType::Chat => router.route(
                    "/chat/completions",
                    post(move |state, payload| chat_completions(state, payload, models)),
                ),
                PipelineType::Completion => router.route(
                    "/completions",
                    post(move |state, payload| completions(state, payload, models)),
                ),
                PipelineType::Embeddings => router.route(
                    "/embeddings",
                    post(move |state, payload| embeddings(state, payload, models)),
                ),
            },
            _ => router,
        };
    }

    return router.with_state(Arc::new(model_registry.clone()));
}

pub async fn chat_completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<ChatCompletionRequest>,
    model_keys: Vec<String>,
) -> Result<Json<ChatCompletionResponse>, StatusCode> {
    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.chat_completions(payload.clone()).await.unwrap();

            return Ok(Json(response));
        }
    }

    Err(StatusCode::SERVICE_UNAVAILABLE)
}

pub async fn completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<CompletionRequest>,
    model_keys: Vec<String>,
) -> Result<Json<CompletionResponse>, StatusCode> {
    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.completions(payload.clone()).await.unwrap();

            return Ok(Json(response));
        }
    }

    Err(StatusCode::SERVICE_UNAVAILABLE)
}

pub async fn embeddings(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<EmbeddingsRequest>,
    model_keys: Vec<String>,
) -> Result<Json<EmbeddingsResponse>, StatusCode> {
    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.embeddings(payload.clone()).await.unwrap();

            return Ok(Json(response));
        }
    }

    Err(StatusCode::SERVICE_UNAVAILABLE)
}
