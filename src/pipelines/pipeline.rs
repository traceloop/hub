use std::collections::HashMap;
use std::sync::Arc;

use crate::config::models::PipelineType;
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse};
use crate::{
    ai_models::registry::ModelRegistry,
    config::models::{Pipeline, PluginConfig},
    models::chat::{ChatCompletionRequest, ChatCompletionResponse},
};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use opentelemetry::global::ObjectSafeSpan;
use opentelemetry::trace::{SpanKind, Status};
use opentelemetry::KeyValue;
use opentelemetry::{global, trace::Tracer};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_semantic_conventions::attribute::GEN_AI_REQUEST_MODEL;
use opentelemetry_semantic_conventions::trace::{
    GEN_AI_REQUEST_FREQUENCY_PENALTY, GEN_AI_REQUEST_PRESENCE_PENALTY, GEN_AI_REQUEST_TEMPERATURE,
    GEN_AI_REQUEST_TOP_P, GEN_AI_RESPONSE_ID, GEN_AI_RESPONSE_MODEL,
};

pub fn create_pipeline(pipeline: &Pipeline, model_registry: &ModelRegistry) -> Router {
    let mut router = Router::new();

    for plugin in pipeline.plugins.clone() {
        router = match plugin {
            PluginConfig::Tracing { endpoint, api_key } => {
                global::set_text_map_propagator(TraceContextPropagator::new());
                let mut headers = HashMap::new();
                headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));

                let exporter: SpanExporter = SpanExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint)
                    .with_headers(headers)
                    .build()
                    .expect("Failed to initialize OpenTelemetry");
                let provider = TracerProvider::builder()
                    .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
                    .build();
                global::set_tracer_provider(provider);

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
    let tracer = global::tracer("traceloop_hub");
    let mut span = tracer
        .span_builder("traceloop_hub.chat")
        .with_kind(SpanKind::Client)
        .start(&tracer);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            span.set_attribute(KeyValue::new("llm.request.type", "chat"));
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_MODEL,
                model.model_type.clone(),
            ));
            if let Some(frequency_penalty) = payload.frequency_penalty {
                span.set_attribute(KeyValue::new(
                    GEN_AI_REQUEST_FREQUENCY_PENALTY,
                    frequency_penalty as f64,
                ));
            }
            if let Some(presence_penalty) = payload.presence_penalty {
                span.set_attribute(KeyValue::new(
                    GEN_AI_REQUEST_PRESENCE_PENALTY,
                    presence_penalty as f64,
                ));
            }
            if let Some(top_p) = payload.top_p {
                span.set_attribute(KeyValue::new(GEN_AI_REQUEST_TOP_P, top_p as f64));
            }
            if let Some(temperature) = payload.temperature {
                span.set_attribute(KeyValue::new(
                    GEN_AI_REQUEST_TEMPERATURE,
                    temperature as f64,
                ));
            }

            for (i, message) in payload.messages.iter().enumerate() {
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.prompt.{}.role", i),
                    message.role.clone(),
                ));
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.prompt.{}.content", i),
                    message.content.clone(),
                ));
            }

            let response = model.chat_completions(payload.clone()).await.unwrap();

            for message in response.choices.iter() {
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.role", message.index),
                    message.message.role.clone(),
                ));
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.content", message.index),
                    message.message.content.clone(),
                ));
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.finish_reason", message.index),
                    message.finish_reason.clone().unwrap_or_default(),
                ));
            }
            span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_ID, response.id.clone()));
            span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, response.model.clone()));
            span.set_attribute(KeyValue::new(
                "gen_ai.usage.prompt_tokens",
                response.usage.prompt_tokens as i64,
            ));
            span.set_attribute(KeyValue::new(
                "gen_ai.usage.completion_tokens",
                response.usage.completion_tokens as i64,
            ));
            span.set_attribute(KeyValue::new(
                "gen_ai.usage.total_tokens",
                response.usage.total_tokens as i64,
            ));

            span.set_status(Status::Ok);
            return Ok(Json(response));
        }
    }

    span.set_status(Status::error("Not Found"));
    Err(StatusCode::NOT_FOUND)
}

pub async fn completions(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<CompletionRequest>,
    model_keys: Vec<String>,
) -> Result<Json<CompletionResponse>, StatusCode> {
    let tracer = global::tracer("traceloop_hub");
    let mut span = tracer
        .span_builder("traceloop_hub.completion")
        .with_kind(SpanKind::Client)
        .start(&tracer);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            span.set_attribute(KeyValue::new("llm.request.type", "completion"));
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_MODEL,
                model.model_type.clone(),
            ));
            if let Some(frequency_penalty) = payload.frequency_penalty {
                span.set_attribute(KeyValue::new(
                    GEN_AI_REQUEST_FREQUENCY_PENALTY,
                    frequency_penalty as f64,
                ));
            }
            if let Some(presence_penalty) = payload.presence_penalty {
                span.set_attribute(KeyValue::new(
                    GEN_AI_REQUEST_PRESENCE_PENALTY,
                    presence_penalty as f64,
                ));
            }
            if let Some(top_p) = payload.top_p {
                span.set_attribute(KeyValue::new(GEN_AI_REQUEST_TOP_P, top_p as f64));
            }
            if let Some(temperature) = payload.temperature {
                span.set_attribute(KeyValue::new(
                    GEN_AI_REQUEST_TEMPERATURE,
                    temperature as f64,
                ));
            }

            span.set_attribute(KeyValue::new("gen_ai.prompt", payload.prompt.clone()));

            let response = model.completions(payload.clone()).await.unwrap();

            for choice in response.choices.iter() {
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.role", choice.index),
                    "assistant".to_string(),
                ));
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.content", choice.index),
                    choice.text.clone(),
                ));
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.finish_reason", choice.index),
                    choice.finish_reason.clone().unwrap_or_default(),
                ));
            }
            span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_ID, response.id.clone()));
            span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, response.model.clone()));
            span.set_attribute(KeyValue::new(
                "gen_ai.usage.prompt_tokens",
                response.usage.prompt_tokens as i64,
            ));
            span.set_attribute(KeyValue::new(
                "gen_ai.usage.completion_tokens",
                response.usage.completion_tokens as i64,
            ));
            span.set_attribute(KeyValue::new(
                "gen_ai.usage.total_tokens",
                response.usage.total_tokens as i64,
            ));

            span.set_status(Status::Ok);
            return Ok(Json(response));
        }
    }

    span.set_status(Status::error("Not Found"));
    Err(StatusCode::NOT_FOUND)
}

pub async fn embeddings(
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<EmbeddingsRequest>,
    model_keys: Vec<String>,
) -> Result<Json<EmbeddingsResponse>, StatusCode> {
    let tracer = global::tracer("traceloop_hub");
    let mut span = tracer
        .span_builder("traceloop_hub.embeddings")
        .with_kind(SpanKind::Client)
        .start(&tracer);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            span.set_attribute(KeyValue::new("llm.request.type", "embeddings"));
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_MODEL,
                model.model_type.clone(),
            ));

            // Add input text attributes
            match &payload.input {
                EmbeddingsInput::Single(text) => {
                    span.set_attribute(KeyValue::new("llm.prompt.0.content", text.clone()));
                }
                EmbeddingsInput::Multiple(texts) => {
                    for (i, text) in texts.iter().enumerate() {
                        span.set_attribute(KeyValue::new(
                            format!("llm.prompt.{}.role", i),
                            "user".to_string(),
                        ));
                        span.set_attribute(KeyValue::new(
                            format!("llm.prompt.{}.content", i),
                            text.clone(),
                        ));
                    }
                }
            }

            let response = model.embeddings(payload.clone()).await.unwrap();

            span.set_attribute(KeyValue::new(
                "gen_ai.usage.prompt_tokens",
                response.usage.prompt_tokens as i64,
            ));
            span.set_attribute(KeyValue::new(
                "gen_ai.usage.total_tokens",
                response.usage.total_tokens as i64,
            ));

            span.set_status(Status::Ok);
            return Ok(Json(response));
        }
    }

    span.set_status(Status::error("Not Found"));
    Err(StatusCode::NOT_FOUND)
}
