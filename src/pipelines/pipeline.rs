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
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use reqwest_streams::error::StreamBodyError;
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

    let matching_models: Vec<_> = model_keys
        .iter()
        .filter_map(|key| {
            let model = model_registry.get(key)?;
            if payload.model == model.model_type {
                Some((key.clone(), model))
            } else {
                None
            }
        })
        .collect();

    if matching_models.is_empty() {
        tracer.log_error("No matching model found".to_string());
        eprintln!("No matching model found for: {}", payload.model);
        return Err(StatusCode::NOT_FOUND);
    }

    let mut last_error = None;

    for (model_key, model) in matching_models {
        match model.chat_completions(payload.clone()).await {
            Ok(response) => match response {
                ChatCompletionResponse::NonStream(completion) => {
                    tracer.log_success(&completion);
                    return Ok(Json(completion).into_response());
                }
                ChatCompletionResponse::Stream(stream) => {
                    return Ok(Sse::new(trace_and_stream(tracer, stream))
                        .keep_alive(KeepAlive::default())
                        .into_response());
                }
            },
            Err(status_code) => {
                eprintln!(
                    "Chat completion error for model {}: {:?}",
                    model_key, status_code
                );

                if is_transient_error(status_code) {
                    eprintln!(
                        "Transient error for model {}, trying next model...",
                        model_key
                    );
                    last_error = Some(status_code);
                    continue;
                } else {
                    return Err(status_code);
                }
            }
        }
    }

    if let Some(error) = last_error {
        tracer.log_error(format!("All models failed with error: {}", error));
        Err(error)
    } else {
        tracer.log_error("No matching model found".to_string());
        eprintln!("No matching model found for: {}", payload.model);
        Err(StatusCode::NOT_FOUND)
    }
}

fn is_transient_error(status_code: StatusCode) -> bool {
    matches!(
        status_code,
        StatusCode::TOO_MANY_REQUESTS | // 429
        StatusCode::REQUEST_TIMEOUT |   // 408
        StatusCode::SERVICE_UNAVAILABLE | // 503
        StatusCode::BAD_GATEWAY |      // 502
        StatusCode::GATEWAY_TIMEOUT // 504
    )
}

pub async fn completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<CompletionRequest>,
    model_keys: Vec<String>,
) -> impl IntoResponse {
    let mut tracer = OtelTracer::start("completion", &payload);

    let matching_models: Vec<_> = model_keys
        .iter()
        .filter_map(|key| {
            let model = model_registry.get(key)?;
            if payload.model == model.model_type {
                Some((key.clone(), model))
            } else {
                None
            }
        })
        .collect();

    if matching_models.is_empty() {
        tracer.log_error("No matching model found".to_string());
        eprintln!("No matching model found for: {}", payload.model);
        return Err(StatusCode::NOT_FOUND);
    }

    let mut last_error = None;

    for (model_key, model) in matching_models {
        match model.completions(payload.clone()).await {
            Ok(response) => {
                tracer.log_success(&response);
                return Ok(Json(response));
            }
            Err(status_code) => {
                eprintln!(
                    "Completion error for model {}: {:?}",
                    model_key, status_code
                );

                if is_transient_error(status_code) {
                    eprintln!(
                        "Transient error for model {}, trying next model...",
                        model_key
                    );
                    last_error = Some(status_code);
                    continue;
                } else {
                    return Err(status_code);
                }
            }
        }
    }

    if let Some(error) = last_error {
        tracer.log_error(format!("All models failed with error: {}", error));
        Err(error)
    } else {
        tracer.log_error("No matching model found".to_string());
        eprintln!("No matching model found for: {}", payload.model);
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn embeddings(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<EmbeddingsRequest>,
    model_keys: Vec<String>,
) -> impl IntoResponse {
    let mut tracer = OtelTracer::start("embeddings", &payload);

    let matching_models: Vec<_> = model_keys
        .iter()
        .filter_map(|key| {
            let model = model_registry.get(key)?;
            if payload.model == model.model_type {
                Some((key.clone(), model))
            } else {
                None
            }
        })
        .collect();

    if matching_models.is_empty() {
        tracer.log_error("No matching model found".to_string());
        eprintln!("No matching model found for: {}", payload.model);
        return Err(StatusCode::NOT_FOUND);
    }

    let mut last_error = None;

    for (model_key, model) in matching_models {
        match model.embeddings(payload.clone()).await {
            Ok(response) => {
                tracer.log_success(&response);
                return Ok(Json(response));
            }
            Err(status_code) => {
                eprintln!(
                    "Embeddings error for model {}: {:?}",
                    model_key, status_code
                );

                if is_transient_error(status_code) {
                    eprintln!(
                        "Transient error for model {}, trying next model...",
                        model_key
                    );
                    last_error = Some(status_code);
                    continue;
                } else {
                    return Err(status_code);
                }
            }
        }
    }

    if let Some(error) = last_error {
        tracer.log_error(format!("All models failed with error: {}", error));
        Err(error)
    } else {
        tracer.log_error("No matching model found".to_string());
        eprintln!("No matching model found for: {}", payload.model);
        Err(StatusCode::NOT_FOUND)
    }
}
