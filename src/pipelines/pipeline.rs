use crate::config::models::PipelineType;
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::pipelines::otel::OtelTracer;
use crate::{
    ai_models::registry::ModelRegistry,
    config::models::{Pipeline, PluginConfig},
    models::chat::{ChatCompletionRequest, ChatCompletionResponse},
};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use std::sync::Arc;

pub fn create_pipeline(pipeline: &Pipeline, model_registry: &ModelRegistry) -> Router {
    let mut router = Router::new();

    for plugin in pipeline.plugins.clone() {
        router = match plugin {
            PluginConfig::Tracing { endpoint, api_key } => {
                OtelTracer::init(endpoint, api_key);
                router
            }
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

    router.with_state(Arc::new(model_registry.clone()))
}

pub async fn chat_completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<ChatCompletionRequest>,
    model_keys: Vec<String>,
) -> Result<Json<ChatCompletionResponse>, StatusCode> {
    let mut tracer = OtelTracer::start("chat", &payload);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.chat_completions(payload.clone()).await.unwrap();
            tracer.log_success(&response);
            return Ok(Json(response));
        }
    }

    tracer.log_error();
    Err(StatusCode::NOT_FOUND)
}

pub async fn completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<CompletionRequest>,
    model_keys: Vec<String>,
) -> Result<Json<CompletionResponse>, StatusCode> {
    let mut tracer = OtelTracer::start("completion", &payload);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.completions(payload.clone()).await.unwrap();
            tracer.log_success(&response);
            return Ok(Json(response));
        }
    }

    tracer.log_error();
    Err(StatusCode::NOT_FOUND)
}

pub async fn embeddings(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<EmbeddingsRequest>,
    model_keys: Vec<String>,
) -> Result<Json<EmbeddingsResponse>, StatusCode> {
    let mut tracer = OtelTracer::start("embeddings", &payload);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.embeddings(payload.clone()).await.unwrap();
            tracer.log_success(&response);
            return Ok(Json(response));
        }
    }

    tracer.log_error();
    Err(StatusCode::NOT_FOUND)
}
