use crate::config::models::PipelineType;
use crate::models::chat::ChatCompletionResponse;
use crate::models::completion::CompletionRequest;
use crate::models::embeddings::EmbeddingsRequest;
use crate::models::streaming::ChatCompletionChunk;
use crate::pipelines::otel::OtelTracer;
use crate::{
    ai_models::registry::ModelRegistry,
    config::models::{Pipeline, PluginConfig},
    models::chat::ChatCompletionRequest,
};
use async_stream::stream;
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Sse};
use axum::{extract::State, http::StatusCode, routing::{post, get}, Json, Router};
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use reqwest_streams::error::StreamBodyError;
use std::sync::Arc;

pub fn create_pipeline(pipeline: &Pipeline, model_registry: &ModelRegistry) -> Router {
    let mut router = Router::new();
    
    let available_models: Vec<String> = pipeline.plugins.iter()
        .find_map(|plugin| {
            if let PluginConfig::ModelRouter { models } = plugin {
                Some(models.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();

    router = router.route(
        "/models",
        get(move |State(model_registry): State<Arc<ModelRegistry>>| async move {
            let model_info = model_registry.get_filtered_model_info(&available_models);
            Json(model_info)
        }),
    );

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

fn trace_and_stream(
    mut tracer: OtelTracer,
    stream: BoxStream<'static, Result<ChatCompletionChunk, StreamBodyError>>,
) -> impl Stream<Item = Result<Event, axum::Error>> {
    stream! {
        let mut stream = stream;
        while let Some(result) = stream.next().await {
            yield match result {
                Ok(chunk) => {
                    tracer.log_chunk(&chunk);
                    Event::default().json_data(chunk)
                }
                Err(e) => {
                    eprintln!("Error in stream: {:?}", e);
                    tracer.log_error(e.to_string());
                    Err(axum::Error::new(e))
                }
            };
        }
        tracer.streaming_end();
    }
}

pub async fn chat_completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<ChatCompletionRequest>,
    model_keys: Vec<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut tracer = OtelTracer::start("chat", &payload);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model
                .chat_completions(payload.clone())
                .await
                .inspect_err(|e| {
                    eprintln!("Chat completion error for model {}: {:?}", model_key, e);
                })?;

            if let ChatCompletionResponse::NonStream(completion) = response {
                tracer.log_success(&completion);
                return Ok(Json(completion).into_response());
            }

            if let ChatCompletionResponse::Stream(stream) = response {
                return Ok(Sse::new(trace_and_stream(tracer, stream))
                    .keep_alive(KeepAlive::default())
                    .into_response());
            }
        }
    }

    tracer.log_error("No matching model found".to_string());
    eprintln!("No matching model found for: {}", payload.model);
    Err(StatusCode::NOT_FOUND)
}

pub async fn completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<CompletionRequest>,
    model_keys: Vec<String>,
) -> impl IntoResponse {
    let mut tracer = OtelTracer::start("completion", &payload);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.completions(payload.clone()).await.inspect_err(|e| {
                eprintln!("Completion error for model {}: {:?}", model_key, e);
            })?;
            tracer.log_success(&response);
            return Ok(Json(response));
        }
    }

    tracer.log_error("No matching model found".to_string());
    eprintln!("No matching model found for: {}", payload.model);
    Err(StatusCode::NOT_FOUND)
}

pub async fn embeddings(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<EmbeddingsRequest>,
    model_keys: Vec<String>,
) -> impl IntoResponse {
    let mut tracer = OtelTracer::start("embeddings", &payload);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            let response = model.embeddings(payload.clone()).await.inspect_err(|e| {
                eprintln!("Embeddings error for model {}: {:?}", model_key, e);
            })?;
            tracer.log_success(&response);
            return Ok(Json(response));
        }
    }

    tracer.log_error("No matching model found".to_string());
    eprintln!("No matching model found for: {}", payload.model);
    Err(StatusCode::NOT_FOUND)
}
