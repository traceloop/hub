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
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use reqwest_streams::error::StreamBodyError;
use std::sync::Arc;

pub fn create_pipeline(pipeline: &Pipeline, model_registry: &ModelRegistry) -> Router {
    let mut router = Router::new();

    let available_models: Vec<String> = pipeline
        .plugins
        .iter()
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
        get(
            move |State(model_registry): State<Arc<ModelRegistry>>| async move {
                let model_info = model_registry.get_filtered_model_info(&available_models);
                Json(model_info)
            },
        ),
    );

    for plugin in pipeline.plugins.clone() {
        router = match plugin {
            PluginConfig::Tracing { endpoint, api_key } => {
                tracing::info!("Initializing OtelTracer for pipeline {}", pipeline.name);
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
                    eprintln!("Error in stream: {e:?}");
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
                    eprintln!("Chat completion error for model {model_key}: {e:?}");
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
                eprintln!("Completion error for model {model_key}: {e:?}");
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
                eprintln!("Embeddings error for model {model_key}: {e:?}");
            })?;
            tracer.log_success(&response);
            return Ok(Json(response));
        }
    }

    tracer.log_error("No matching model found".to_string());
    eprintln!("No matching model found for: {}", payload.model);
    Err(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ai_models::registry::ModelRegistry,
        config::models::{
            ModelConfig, Pipeline, PipelineType, PluginConfig, Provider as ProviderConfig,
        },
        providers::provider::Provider,
        providers::registry::ProviderRegistry,
    };
    use axum::{
        async_trait,
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct MockProvider {
        key: String,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn new(config: &ProviderConfig) -> Self {
            Self {
                key: config.key.clone(),
            }
        }

        fn key(&self) -> String {
            self.key.clone()
        }

        fn r#type(&self) -> String {
            "mock".to_string()
        }

        async fn chat_completions(
            &self,
            _payload: crate::models::chat::ChatCompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::chat::ChatCompletionResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }

        async fn completions(
            &self,
            _payload: crate::models::completion::CompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::completion::CompletionResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }

        async fn embeddings(
            &self,
            _payload: crate::models::embeddings::EmbeddingsRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::embeddings::EmbeddingsResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }
    }

    // Helper function to create mock provider and registry
    fn create_test_provider_registry() -> Arc<ProviderRegistry> {
        let provider_config = ProviderConfig {
            key: "test-provider".to_string(),
            r#type: "openai".to_string(),
            api_key: String::new(),
            params: HashMap::new(),
        };
        Arc::new(ProviderRegistry::new(&[provider_config]).unwrap())
    }

    // Helper function to create model configs
    fn create_model_configs(model_keys: Vec<&str>) -> Vec<ModelConfig> {
        model_keys
            .into_iter()
            .map(|key| ModelConfig {
                key: key.to_string(),
                r#type: "test".to_string(),
                provider: "test-provider".to_string(),
                params: HashMap::new(),
            })
            .collect()
    }

    // Helper function to create test pipeline
    fn create_test_pipeline(model_keys: Vec<&str>) -> Pipeline {
        Pipeline {
            name: "test".to_string(),
            r#type: PipelineType::Chat,
            plugins: vec![PluginConfig::ModelRouter {
                models: model_keys.into_iter().map(|s| s.to_string()).collect(),
            }],
        }
    }

    // Helper function to make GET request to /models
    async fn get_models_response(app: Router) -> serde_json::Value {
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/models")
                    .method("GET")
                    .header("x-traceloop-pipeline", "test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn test_models_endpoint() {
        let provider_registry = create_test_provider_registry();
        let model_configs = create_model_configs(vec!["test-model"]);
        let model_registry = ModelRegistry::new(&model_configs, provider_registry).unwrap();
        let pipeline = create_test_pipeline(vec!["test-model"]);
        let app = create_pipeline(&pipeline, &model_registry);

        let response = get_models_response(app).await;

        assert_eq!(response["object"], "list");
        assert!(response["data"].is_array());
        let data = response["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);

        let model = &data[0];
        assert_eq!(model["id"], "test-model");
        assert_eq!(model["object"], "model");
        assert_eq!(model["owned_by"], "test-provider");
    }
    #[tokio::test]
    async fn test_models_endpoint_multiple_providers() {
        let provider_config_1 = ProviderConfig {
            key: "test-provider-1".to_string(),
            r#type: "openai".to_string(),
            api_key: String::new(),
            params: HashMap::new(),
        };
        let provider_config_2 = ProviderConfig {
            key: "test-provider-2".to_string(),
            r#type: "openai".to_string(),
            api_key: String::new(),
            params: HashMap::new(),
        };
        let provider_registry =
            Arc::new(ProviderRegistry::new(&[provider_config_1, provider_config_2]).unwrap());

        let model_configs = vec![
            ModelConfig {
                key: "test-model-1".to_string(),
                r#type: "test".to_string(),
                provider: "test-provider-1".to_string(),
                params: HashMap::new(),
            },
            ModelConfig {
                key: "test-model-2".to_string(),
                r#type: "test".to_string(),
                provider: "test-provider-2".to_string(),
                params: HashMap::new(),
            },
        ];

        let model_registry =
            Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
        let pipeline = create_test_pipeline(vec!["test-model-1", "test-model-2"]);
        let app = create_pipeline(&pipeline, &model_registry);

        let response = get_models_response(app).await;

        assert_eq!(response["object"], "list");
        assert!(response["data"].is_array());
        let data = response["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);

        let model_1 = data.iter().find(|m| m["id"] == "test-model-1").unwrap();
        let model_2 = data.iter().find(|m| m["id"] == "test-model-2").unwrap();

        assert_eq!(model_1["owned_by"], "test-provider-1");
        assert_eq!(model_2["owned_by"], "test-provider-2");
    }

    #[tokio::test]
    async fn test_models_endpoint_empty_models() {
        let provider_registry = create_test_provider_registry();
        let model_configs = create_model_configs(vec![]);
        let model_registry =
            Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
        let pipeline = create_test_pipeline(vec![]);
        let app = create_pipeline(&pipeline, &model_registry);

        let response = get_models_response(app).await;

        assert_eq!(response["object"], "list");
        assert!(response["data"].is_array());
        let data = response["data"].as_array().unwrap();
        assert_eq!(data.len(), 0);
    }

    #[tokio::test]
    async fn test_models_endpoint_filtered_models() {
        let provider_registry = create_test_provider_registry();
        let model_configs =
            create_model_configs(vec!["test-model-1", "test-model-2", "test-model-3"]);
        let model_registry =
            Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
        let pipeline = create_test_pipeline(vec!["test-model-1", "test-model-3"]); // Only include 2 of 3 models
        let app = create_pipeline(&pipeline, &model_registry);

        let response = get_models_response(app).await;

        assert_eq!(response["object"], "list");
        assert!(response["data"].is_array());
        let data = response["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);

        let ids: Vec<_> = data.iter().map(|m| m["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"test-model-1"));
        assert!(ids.contains(&"test-model-3"));
        assert!(!ids.contains(&"test-model-2"));
    }
}
