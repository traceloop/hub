use crate::config::models::PipelineType;
use crate::guardrails::middleware::GuardrailsLayer;
use crate::guardrails::types::GuardrailResources;
use crate::models::chat::ChatCompletionResponse;
use crate::models::completion::CompletionRequest;
use crate::models::embeddings::EmbeddingsRequest;
use crate::models::streaming::ChatCompletionChunk;
use crate::pipelines::otel::{OtelTracer, SharedTracer};
use crate::pipelines::tracing_middleware::TracingLayer;
use crate::providers::provider::get_vendor_name;
use crate::{
    ai_models::registry::ModelRegistry,
    config::models::{Pipeline, PluginConfig},
    models::chat::ChatCompletionRequest,
};
use async_stream::stream;
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Response, Sse};
use axum::{
    Json, Router,
    extract::{Extension, State},
    http::StatusCode,
    routing::{get, post},
};
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use reqwest_streams::error::StreamBodyError;
use std::sync::Arc;

// Re-export builder and orchestrator functions for backward compatibility with tests
pub use crate::guardrails::runner::{blocked_response, warning_header_value};
pub use crate::guardrails::setup::{
    build_guardrail_resources, build_pipeline_guardrails, resolve_guard_defaults,
};

pub fn create_pipeline(
    pipeline: &Pipeline,
    model_registry: &ModelRegistry,
    guardrail_resources: Option<&GuardrailResources>,
) -> Router {
    let guardrails =
        guardrail_resources.map(|shared| build_pipeline_guardrails(shared, &pipeline.guards));
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
                    post(move |tracer, state, payload| {
                        chat_completions(tracer, state, payload, models)
                    }),
                ),
                PipelineType::Completion => router.route(
                    "/completions",
                    post(move |tracer, state, payload| completions(tracer, state, payload, models)),
                ),
                PipelineType::Embeddings => router.route(
                    "/embeddings",
                    post(move |tracer, state, payload| embeddings(tracer, state, payload, models)),
                ),
            },
            _ => router,
        };
    }

    router
        .with_state(Arc::new(model_registry.clone()))
        .layer(GuardrailsLayer::new(guardrails))
        .layer(TracingLayer::new())
}

fn trace_and_stream(
    tracer: SharedTracer,
    stream: BoxStream<'static, Result<ChatCompletionChunk, StreamBodyError>>,
) -> impl Stream<Item = Result<Event, axum::Error>> {
    stream! {
        let mut stream = stream;
        while let Some(result) = stream.next().await {
            yield match result {
                Ok(chunk) => {
                    tracer.lock().unwrap().log_chunk(&chunk);
                    Event::default().json_data(chunk)
                }
                Err(e) => {
                    eprintln!("Error in stream: {e:?}");
                    tracer.lock().unwrap().log_error(e.to_string());
                    Err(axum::Error::new(e))
                }
            };
        }
        tracer.lock().unwrap().streaming_end();
    }
}

pub async fn chat_completions(
    Extension(tracer): Extension<SharedTracer>,
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<ChatCompletionRequest>,
    model_keys: Vec<String>,
) -> Result<Response, StatusCode> {
    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            {
                let mut tracer_guard = tracer.lock().unwrap();
                tracer_guard.start_llm_span("chat", &payload);
                tracer_guard.set_vendor(&get_vendor_name(&model.provider.r#type()));
            }

            let response = match model.chat_completions(payload.clone()).await {
                Ok(response) => response,
                Err(e) => {
                    eprintln!("Chat completion error for model {model_key}: {e:?}");
                    tracer
                        .lock()
                        .unwrap()
                        .log_error(format!("Chat completion failed: {e:?}"));
                    return Err(e);
                }
            };

            if let ChatCompletionResponse::NonStream(completion) = response {
                tracer.lock().unwrap().log_success(&completion);
                return Ok(Json(completion).into_response());
            }

            if let ChatCompletionResponse::Stream(stream) = response {
                return Ok(Sse::new(trace_and_stream(tracer, stream))
                    .keep_alive(KeepAlive::default())
                    .into_response());
            }
        }
    }

    tracer
        .lock()
        .unwrap()
        .log_error("No matching model found".to_string());
    eprintln!("No matching model found for: {}", payload.model);
    Err(StatusCode::NOT_FOUND)
}

pub async fn completions(
    Extension(tracer): Extension<SharedTracer>,
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<CompletionRequest>,
    model_keys: Vec<String>,
) -> Result<Response, StatusCode> {
    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            {
                let mut tracer_guard = tracer.lock().unwrap();
                tracer_guard.start_llm_span("completion", &payload);
                tracer_guard.set_vendor(&get_vendor_name(&model.provider.r#type()));
            }

            let response = match model.completions(payload.clone()).await {
                Ok(response) => response,
                Err(e) => {
                    eprintln!("Completion error for model {model_key}: {e:?}");
                    tracer
                        .lock()
                        .unwrap()
                        .log_error(format!("Completion failed: {e:?}"));
                    return Err(e);
                }
            };
            tracer.lock().unwrap().log_success(&response);

            return Ok(Json(response).into_response());
        }
    }

    tracer
        .lock()
        .unwrap()
        .log_error("No matching model found".to_string());
    eprintln!("No matching model found for: {}", payload.model);
    Err(StatusCode::NOT_FOUND)
}

pub async fn embeddings(
    Extension(tracer): Extension<SharedTracer>,
    State(model_registry): State<Arc<ModelRegistry>>,
    Json(payload): Json<EmbeddingsRequest>,
    model_keys: Vec<String>,
) -> impl IntoResponse {
    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            {
                let mut tracer_guard = tracer.lock().unwrap();
                tracer_guard.start_llm_span("embeddings", &payload);
                tracer_guard.set_vendor(&get_vendor_name(&model.provider.r#type()));
            }

            let response = match model.embeddings(payload.clone()).await {
                Ok(response) => response,
                Err(e) => {
                    eprintln!("Embeddings error for model {model_key}: {e:?}");
                    tracer
                        .lock()
                        .unwrap()
                        .log_error(format!("Embeddings failed: {e:?}"));
                    return Err(e);
                }
            };
            tracer.lock().unwrap().log_success(&response);
            return Ok(Json(response));
        }
    }

    tracer
        .lock()
        .unwrap()
        .log_error("No matching model found".to_string());
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
        types::ProviderType,
    };
    use async_trait::async_trait;
    use axum::{
        body::{Body, to_bytes},
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

        fn r#type(&self) -> ProviderType {
            ProviderType::OpenAI // Using OpenAI as default for mock
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
            r#type: ProviderType::OpenAI,
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
            guards: vec![],
        }
    }

    // Helper function to create test pipeline with specific type (for otel tests)
    fn create_test_pipeline_with_type(
        model_keys: Vec<&str>,
        pipeline_type: PipelineType,
    ) -> Pipeline {
        Pipeline {
            name: "test".to_string(),
            r#type: pipeline_type,
            plugins: vec![PluginConfig::ModelRouter {
                models: model_keys.into_iter().map(|s| s.to_string()).collect(),
            }],
            guards: vec![],
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
        let app = create_pipeline(&pipeline, &model_registry, None);

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
            r#type: ProviderType::OpenAI,
            api_key: String::new(),
            params: HashMap::new(),
        };
        let provider_config_2 = ProviderConfig {
            key: "test-provider-2".to_string(),
            r#type: ProviderType::OpenAI,
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
        let app = create_pipeline(&pipeline, &model_registry, None);

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
        let app = create_pipeline(&pipeline, &model_registry, None);

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
        let app = create_pipeline(&pipeline, &model_registry, None);

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

    // Test providers with different types for vendor testing
    #[derive(Clone)]
    struct TestProviderOpenAI;
    #[derive(Clone)]
    struct TestProviderAnthropic;
    #[derive(Clone)]
    struct TestProviderAzure;

    #[async_trait]
    impl Provider for TestProviderOpenAI {
        fn new(_config: &ProviderConfig) -> Self {
            Self
        }
        fn key(&self) -> String {
            "openai-key".to_string()
        }
        fn r#type(&self) -> ProviderType {
            ProviderType::OpenAI
        }

        async fn chat_completions(
            &self,
            _payload: crate::models::chat::ChatCompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::chat::ChatCompletionResponse, StatusCode> {
            Ok(crate::models::chat::ChatCompletionResponse::NonStream(
                crate::models::chat::ChatCompletion {
                    id: "test".to_string(),
                    object: None,
                    created: None,
                    model: "gpt-4".to_string(),
                    choices: vec![],
                    usage: crate::models::usage::Usage::default(),
                    system_fingerprint: None,
                },
            ))
        }

        async fn completions(
            &self,
            _payload: CompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::completion::CompletionResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }

        async fn embeddings(
            &self,
            _payload: EmbeddingsRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::embeddings::EmbeddingsResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }
    }

    #[async_trait]
    impl Provider for TestProviderAnthropic {
        fn new(_config: &ProviderConfig) -> Self {
            Self
        }
        fn key(&self) -> String {
            "anthropic-key".to_string()
        }
        fn r#type(&self) -> ProviderType {
            ProviderType::Anthropic
        }

        async fn chat_completions(
            &self,
            _payload: crate::models::chat::ChatCompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::chat::ChatCompletionResponse, StatusCode> {
            Ok(crate::models::chat::ChatCompletionResponse::NonStream(
                crate::models::chat::ChatCompletion {
                    id: "test".to_string(),
                    object: None,
                    created: None,
                    model: "claude-3".to_string(),
                    choices: vec![],
                    usage: crate::models::usage::Usage::default(),
                    system_fingerprint: None,
                },
            ))
        }

        async fn completions(
            &self,
            _payload: CompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::completion::CompletionResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }

        async fn embeddings(
            &self,
            _payload: EmbeddingsRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::embeddings::EmbeddingsResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }
    }

    #[async_trait]
    impl Provider for TestProviderAzure {
        fn new(_config: &ProviderConfig) -> Self {
            Self
        }
        fn key(&self) -> String {
            "azure-key".to_string()
        }
        fn r#type(&self) -> ProviderType {
            ProviderType::Azure
        }

        async fn chat_completions(
            &self,
            _payload: crate::models::chat::ChatCompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::chat::ChatCompletionResponse, StatusCode> {
            Ok(crate::models::chat::ChatCompletionResponse::NonStream(
                crate::models::chat::ChatCompletion {
                    id: "test".to_string(),
                    object: None,
                    created: None,
                    model: "gpt-4".to_string(),
                    choices: vec![],
                    usage: crate::models::usage::Usage::default(),
                    system_fingerprint: None,
                },
            ))
        }

        async fn completions(
            &self,
            _payload: CompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::completion::CompletionResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }

        async fn embeddings(
            &self,
            _payload: EmbeddingsRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::embeddings::EmbeddingsResponse, StatusCode> {
            Err(StatusCode::NOT_IMPLEMENTED)
        }
    }

    #[test]
    fn test_vendor_mapping_integration() {
        // Test that different provider types map to correct vendor names
        // This tests the integration between provider types and vendor names
        assert_eq!(get_vendor_name(&ProviderType::OpenAI), "openai");
        assert_eq!(get_vendor_name(&ProviderType::Anthropic), "Anthropic");
        assert_eq!(get_vendor_name(&ProviderType::Azure), "Azure");
        assert_eq!(get_vendor_name(&ProviderType::Bedrock), "AWS");
        assert_eq!(get_vendor_name(&ProviderType::VertexAI), "Google");
    }

    #[test]
    fn test_provider_type_methods() {
        // Test that our test providers return the correct types
        // This ensures the pipeline would call set_vendor with the right values
        let openai_provider = TestProviderOpenAI;
        let anthropic_provider = TestProviderAnthropic;
        let azure_provider = TestProviderAzure;

        assert_eq!(openai_provider.r#type(), ProviderType::OpenAI);
        assert_eq!(anthropic_provider.r#type(), ProviderType::Anthropic);
        assert_eq!(azure_provider.r#type(), ProviderType::Azure);

        // Test that these map to the correct vendor names
        assert_eq!(get_vendor_name(&openai_provider.r#type()), "openai");
        assert_eq!(get_vendor_name(&anthropic_provider.r#type()), "Anthropic");
        assert_eq!(get_vendor_name(&azure_provider.r#type()), "Azure");
    }

    // OpenTelemetry span verification tests
    mod otel_span_tests {
        use super::*;
        use opentelemetry::trace::{SpanKind, Status as OtelStatus};
        use opentelemetry_sdk::export::trace::SpanData;
        use opentelemetry_sdk::testing::trace::InMemorySpanExporter;
        use opentelemetry_sdk::trace::TracerProvider;
        use std::sync::LazyLock;

        /// Shared OTel exporter, initialized once for all span tests
        /// Tests are isolated by tracking span count before/after each request
        static TEST_EXPORTER: LazyLock<InMemorySpanExporter> = LazyLock::new(|| {
            let exporter = InMemorySpanExporter::default();
            let provider = TracerProvider::builder()
                .with_simple_exporter(exporter.clone())
                .build();
            opentelemetry::global::set_tracer_provider(provider);
            exporter
        });

        // Mock provider that returns realistic responses with Usage data
        #[derive(Clone)]
        struct MockProviderForSpanTests {
            provider_type: ProviderType,
        }

        #[async_trait]
        impl Provider for MockProviderForSpanTests {
            fn new(_config: &ProviderConfig) -> Self {
                Self {
                    provider_type: ProviderType::OpenAI,
                }
            }

            fn key(&self) -> String {
                "test-key".to_string()
            }

            fn r#type(&self) -> ProviderType {
                self.provider_type.clone()
            }

            async fn chat_completions(
                &self,
                payload: crate::models::chat::ChatCompletionRequest,
                _model_config: &ModelConfig,
            ) -> Result<crate::models::chat::ChatCompletionResponse, StatusCode> {
                use crate::models::chat::{
                    ChatCompletion, ChatCompletionChoice, ChatCompletionResponse,
                };
                use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
                use crate::models::usage::Usage;

                Ok(ChatCompletionResponse::NonStream(ChatCompletion {
                    id: "chatcmpl-test123".to_string(),
                    object: Some("chat.completion".to_string()),
                    created: Some(1234567890),
                    model: payload.model.clone(),
                    choices: vec![ChatCompletionChoice {
                        index: 0,
                        message: ChatCompletionMessage {
                            role: "assistant".to_string(),
                            content: Some(ChatMessageContent::String("Test response".to_string())),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                            refusal: None,
                        },
                        finish_reason: Some("stop".to_string()),
                        logprobs: None,
                    }],
                    usage: Usage {
                        prompt_tokens: 10,
                        completion_tokens: 15,
                        total_tokens: 25,
                        completion_tokens_details: None,
                        prompt_tokens_details: None,
                    },
                    system_fingerprint: None,
                }))
            }

            async fn completions(
                &self,
                payload: crate::models::completion::CompletionRequest,
                _model_config: &ModelConfig,
            ) -> Result<crate::models::completion::CompletionResponse, StatusCode> {
                use crate::models::completion::{CompletionChoice, CompletionResponse};
                use crate::models::usage::Usage;

                Ok(CompletionResponse {
                    id: "cmpl-test456".to_string(),
                    object: "text_completion".to_string(),
                    created: 1234567890,
                    model: payload.model.clone(),
                    choices: vec![CompletionChoice {
                        text: "Test completion".to_string(),
                        index: 0,
                        logprobs: None,
                        finish_reason: Some("stop".to_string()),
                    }],
                    usage: Usage {
                        prompt_tokens: 5,
                        completion_tokens: 10,
                        total_tokens: 15,
                        completion_tokens_details: None,
                        prompt_tokens_details: None,
                    },
                })
            }

            async fn embeddings(
                &self,
                payload: crate::models::embeddings::EmbeddingsRequest,
                _model_config: &ModelConfig,
            ) -> Result<crate::models::embeddings::EmbeddingsResponse, StatusCode> {
                use crate::models::embeddings::{Embedding, Embeddings, EmbeddingsResponse};
                use crate::models::usage::EmbeddingUsage;

                Ok(EmbeddingsResponse {
                    object: "list".to_string(),
                    data: vec![Embeddings {
                        object: "embedding".to_string(),
                        embedding: Embedding::Float(vec![0.1, 0.2, 0.3]),
                        index: 0,
                    }],
                    model: payload.model.clone(),
                    usage: EmbeddingUsage {
                        prompt_tokens: Some(8),
                        total_tokens: Some(8),
                    },
                })
            }
        }

        // Mock provider that returns errors
        #[derive(Clone)]
        struct MockProviderError {
            provider_type: ProviderType,
        }

        #[async_trait]
        impl Provider for MockProviderError {
            fn new(_config: &ProviderConfig) -> Self {
                Self {
                    provider_type: ProviderType::OpenAI,
                }
            }

            fn key(&self) -> String {
                "test-key".to_string()
            }

            fn r#type(&self) -> ProviderType {
                self.provider_type.clone()
            }

            async fn chat_completions(
                &self,
                _payload: crate::models::chat::ChatCompletionRequest,
                _model_config: &ModelConfig,
            ) -> Result<crate::models::chat::ChatCompletionResponse, StatusCode> {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }

            async fn completions(
                &self,
                _payload: crate::models::completion::CompletionRequest,
                _model_config: &ModelConfig,
            ) -> Result<crate::models::completion::CompletionResponse, StatusCode> {
                Err(StatusCode::BAD_GATEWAY)
            }

            async fn embeddings(
                &self,
                _payload: crate::models::embeddings::EmbeddingsRequest,
                _model_config: &ModelConfig,
            ) -> Result<crate::models::embeddings::EmbeddingsResponse, StatusCode> {
                Err(StatusCode::SERVICE_UNAVAILABLE)
            }
        }

        // Helper: Create provider registry with specific provider type
        fn create_test_provider_registry_for_spans(
            provider_type: ProviderType,
            return_errors: bool,
        ) -> Arc<ProviderRegistry> {
            let provider_config = ProviderConfig {
                key: "test-provider".to_string(),
                r#type: provider_type.clone(),
                api_key: String::new(),
                params: HashMap::new(),
            };

            let mut registry = ProviderRegistry::new(&[provider_config.clone()]).unwrap();

            // Replace the provider with our mock
            if return_errors {
                let mock = MockProviderError { provider_type };
                let providers = registry.providers_mut();
                providers.insert("test-provider".to_string(), Arc::new(mock));
            } else {
                let mock = MockProviderForSpanTests { provider_type };
                let providers = registry.providers_mut();
                providers.insert("test-provider".to_string(), Arc::new(mock));
            }

            Arc::new(registry)
        }

        // Helper: Collect spans added since before_count
        fn get_spans_for_test(before_count: usize) -> Vec<SpanData> {
            // Get all spans
            let all_spans = TEST_EXPORTER.get_finished_spans().unwrap();

            // Skip the before_count spans and take only the next few
            // We expect exactly 2 spans per test (root + LLM)
            let new_spans: Vec<SpanData> = all_spans.into_iter().skip(before_count).collect();

            // If we have more than 2 spans, try to find the most recent root span
            // and its immediate child
            if new_spans.len() > 2 {
                // Find the last root span (traceloop_hub with Server kind)
                if let Some(root_idx) = new_spans
                    .iter()
                    .rposition(|s| s.name == "traceloop_hub" && s.span_kind == SpanKind::Server)
                {
                    let root = &new_spans[root_idx];
                    let root_trace_id = root.span_context.trace_id();

                    // Collect all spans with the same trace_id
                    return new_spans
                        .into_iter()
                        .filter(|s| s.span_context.trace_id() == root_trace_id)
                        .collect();
                }
            }

            new_spans
        }

        // Helper: Find root span (name="traceloop_hub", SpanKind::Server)
        fn get_root_span(spans: &[SpanData]) -> Option<&SpanData> {
            spans
                .iter()
                .find(|s| s.name == "traceloop_hub" && s.span_kind == SpanKind::Server)
        }

        // Helper: Find LLM span by operation
        fn get_llm_span<'a>(spans: &'a [SpanData], operation: &str) -> Option<&'a SpanData> {
            let expected_name = format!("traceloop_hub.{}", operation);
            spans
                .iter()
                .find(|s| s.name == expected_name && s.span_kind == SpanKind::Client)
        }

        // Helper: Extract attribute value from span
        fn get_span_attribute(span: &SpanData, key: &str) -> Option<String> {
            span.attributes
                .iter()
                .find(|kv| kv.key.to_string() == key)
                .map(|kv| kv.value.to_string())
        }

        // Helper: Check if span is child of another span
        fn is_child_of(child: &SpanData, parent: &SpanData) -> bool {
            child.parent_span_id == parent.span_context.span_id()
                && child.span_context.trace_id() == parent.span_context.trace_id()
        }

        // Helper: Assert span has expected attributes
        fn assert_span_attributes(span: &SpanData, expected: &[(&str, &str)]) {
            for (key, expected_value) in expected {
                let actual = get_span_attribute(span, key);
                assert_eq!(
                    actual.as_deref(),
                    Some(*expected_value),
                    "Attribute {} mismatch. Expected: {}, Got: {:?}",
                    key,
                    expected_value,
                    actual
                );
            }
        }

        #[tokio::test]
        async fn test_chat_completions_success_spans() {
            // Initialize exporter
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            // Create test infrastructure
            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::OpenAI, false);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline = create_test_pipeline(vec!["test-model"]);

            // Create router (includes TracingLayer)
            let app = create_pipeline(&pipeline, &model_registry, None);

            // Prepare request
            use crate::models::chat::ChatCompletionRequest;
            use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
            let request_body = ChatCompletionRequest {
                model: "test".to_string(),
                messages: vec![ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                }],
                temperature: Some(0.7),
                top_p: None,
                n: None,
                stream: None,
                stop: None,
                max_tokens: None,
                max_completion_tokens: None,
                presence_penalty: None,
                frequency_penalty: None,
                logit_bias: None,
                user: None,
                response_format: None,
                tools: None,
                tool_choice: None,
                parallel_tool_calls: None,
                logprobs: None,
                top_logprobs: None,
                reasoning: None,
            };

            // Send request
            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/chat/completions")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            // Verify response success
            assert_eq!(response.status(), StatusCode::OK);

            // Collect new spans
            let spans = get_spans_for_test(before_count);
            assert_eq!(
                spans.len(),
                2,
                "Expected root + LLM span, got {}",
                spans.len()
            );

            // Verify root span
            let root = get_root_span(&spans).expect("Root span not found");
            assert_eq!(root.name, "traceloop_hub");
            assert_eq!(root.span_kind, SpanKind::Server);
            assert_eq!(root.status, OtelStatus::Ok);

            // Verify LLM span
            let llm = get_llm_span(&spans, "chat").expect("LLM span not found");
            assert_eq!(llm.name, "traceloop_hub.chat");
            assert_eq!(llm.span_kind, SpanKind::Client);
            assert_eq!(llm.status, OtelStatus::Ok);

            // Verify hierarchy
            assert!(
                is_child_of(llm, root),
                "LLM span should be child of root span"
            );

            // Verify attributes
            assert_span_attributes(
                llm,
                &[
                    ("gen_ai.system", "openai"),
                    ("gen_ai.request.model", "test"),
                    ("llm.request.type", "chat"),
                ],
            );

            // Verify usage attributes exist
            assert!(
                get_span_attribute(llm, "gen_ai.usage.prompt_tokens").is_some(),
                "prompt_tokens attribute missing"
            );
            assert!(
                get_span_attribute(llm, "gen_ai.usage.completion_tokens").is_some(),
                "completion_tokens attribute missing"
            );
            assert!(
                get_span_attribute(llm, "gen_ai.usage.total_tokens").is_some(),
                "total_tokens attribute missing"
            );
        }

        #[tokio::test]
        async fn test_completions_success_spans() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::Anthropic, false);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline =
                create_test_pipeline_with_type(vec!["test-model"], PipelineType::Completion);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::completion::CompletionRequest;
            let request_body = CompletionRequest {
                model: "test".to_string(),
                prompt: "Test prompt".to_string(),
                max_tokens: Some(50),
                temperature: None,
                top_p: None,
                n: None,
                stream: None,
                logprobs: None,
                echo: None,
                stop: None,
                presence_penalty: None,
                frequency_penalty: None,
                best_of: None,
                logit_bias: None,
                user: None,
                suffix: None,
            };

            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/completions")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);

            let spans = get_spans_for_test(before_count);
            assert_eq!(spans.len(), 2, "Expected root + LLM span");

            let root = get_root_span(&spans).expect("Root span not found");
            let llm = get_llm_span(&spans, "completion").expect("LLM span not found");

            assert_eq!(llm.name, "traceloop_hub.completion");
            assert_eq!(llm.span_kind, SpanKind::Client);
            assert!(is_child_of(llm, root));

            assert_span_attributes(
                llm,
                &[
                    ("gen_ai.system", "Anthropic"),
                    ("gen_ai.request.model", "test"),
                    ("llm.request.type", "completion"),
                ],
            );
        }

        #[tokio::test]
        async fn test_embeddings_success_spans() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::Azure, false);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline =
                create_test_pipeline_with_type(vec!["test-model"], PipelineType::Embeddings);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
            let request_body = EmbeddingsRequest {
                input: EmbeddingsInput::Single("Test text".to_string()),
                model: "test".to_string(),
                encoding_format: None,
                user: None,
            };

            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/embeddings")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);

            let spans = get_spans_for_test(before_count);
            assert_eq!(spans.len(), 2, "Expected root + LLM span");

            let root = get_root_span(&spans).expect("Root span not found");
            let llm = get_llm_span(&spans, "embeddings").expect("LLM span not found");

            assert_eq!(llm.name, "traceloop_hub.embeddings");
            assert_eq!(llm.span_kind, SpanKind::Client);
            assert!(is_child_of(llm, root));

            assert_span_attributes(
                llm,
                &[
                    ("gen_ai.system", "Azure"),
                    ("gen_ai.request.model", "test"),
                    ("llm.request.type", "embeddings"),
                ],
            );
        }

        #[tokio::test]
        async fn test_chat_completions_error_spans() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::OpenAI, true);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline = create_test_pipeline(vec!["test-model"]);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::chat::ChatCompletionRequest;
            use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
            let request_body = ChatCompletionRequest {
                model: "test".to_string(),
                messages: vec![ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                }],
                temperature: None,
                top_p: None,
                n: None,
                stream: None,
                stop: None,
                max_tokens: None,
                max_completion_tokens: None,
                presence_penalty: None,
                frequency_penalty: None,
                logit_bias: None,
                user: None,
                response_format: None,
                tools: None,
                tool_choice: None,
                parallel_tool_calls: None,
                logprobs: None,
                top_logprobs: None,
                reasoning: None,
            };

            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/chat/completions")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            // Verify error response
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

            // Collect spans
            let spans = get_spans_for_test(before_count);
            assert_eq!(spans.len(), 2, "Expected root + LLM span");

            let root = get_root_span(&spans).expect("Root span not found");
            let llm = get_llm_span(&spans, "chat").expect("LLM span not found");

            // Verify error status on both spans
            assert!(
                matches!(root.status, OtelStatus::Error { .. }),
                "Root span should have error status"
            );
            assert!(
                matches!(llm.status, OtelStatus::Error { .. }),
                "LLM span should have error status"
            );

            // Hierarchy should still be correct
            assert!(is_child_of(llm, root));
        }

        #[tokio::test]
        async fn test_completions_error_spans() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::OpenAI, true);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline =
                create_test_pipeline_with_type(vec!["test-model"], PipelineType::Completion);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::completion::CompletionRequest;
            let request_body = CompletionRequest {
                model: "test".to_string(),
                prompt: "Test".to_string(),
                max_tokens: None,
                temperature: None,
                top_p: None,
                n: None,
                stream: None,
                logprobs: None,
                echo: None,
                stop: None,
                presence_penalty: None,
                frequency_penalty: None,
                best_of: None,
                logit_bias: None,
                user: None,
                suffix: None,
            };

            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/completions")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

            let spans = get_spans_for_test(before_count);
            let root = get_root_span(&spans).expect("Root span not found");
            let llm = get_llm_span(&spans, "completion").expect("LLM span not found");

            assert!(matches!(root.status, OtelStatus::Error { .. }));
            assert!(matches!(llm.status, OtelStatus::Error { .. }));
        }

        #[tokio::test]
        async fn test_embeddings_error_spans() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::OpenAI, true);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline =
                create_test_pipeline_with_type(vec!["test-model"], PipelineType::Embeddings);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
            let request_body = EmbeddingsRequest {
                input: EmbeddingsInput::Single("Test".to_string()),
                model: "test".to_string(),
                encoding_format: None,
                user: None,
            };

            let response = app
                .oneshot(
                    Request::builder()
                        .uri("/embeddings")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

            let spans = get_spans_for_test(before_count);
            let root = get_root_span(&spans).expect("Root span not found");
            let llm = get_llm_span(&spans, "embeddings").expect("LLM span not found");

            assert!(matches!(root.status, OtelStatus::Error { .. }));
            assert!(matches!(llm.status, OtelStatus::Error { .. }));
        }

        #[tokio::test]
        async fn test_span_request_attributes_recorded() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::OpenAI, false);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline = create_test_pipeline(vec!["test-model"]);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::chat::ChatCompletionRequest;
            use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
            let request_body = ChatCompletionRequest {
                model: "test".to_string(),
                messages: vec![ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                }],
                temperature: Some(0.8),
                top_p: Some(0.9),
                frequency_penalty: Some(0.5),
                presence_penalty: Some(0.3),
                n: None,
                stream: None,
                stop: None,
                max_tokens: None,
                max_completion_tokens: None,
                logit_bias: None,
                user: None,
                response_format: None,
                tools: None,
                tool_choice: None,
                parallel_tool_calls: None,
                logprobs: None,
                top_logprobs: None,
                reasoning: None,
            };

            let _response = app
                .oneshot(
                    Request::builder()
                        .uri("/chat/completions")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            let spans = get_spans_for_test(before_count);
            let llm = get_llm_span(&spans, "chat").expect("LLM span not found");

            // Verify request parameters are recorded
            assert_span_attributes(llm, &[("gen_ai.request.model", "test")]);

            // Verify float attributes exist (but don't check exact values due to float precision)
            assert!(
                get_span_attribute(llm, "gen_ai.request.temperature").is_some(),
                "Temperature attribute should exist"
            );
            assert!(
                get_span_attribute(llm, "gen_ai.request.top_p").is_some(),
                "top_p attribute should exist"
            );
            assert!(
                get_span_attribute(llm, "gen_ai.request.frequency_penalty").is_some(),
                "frequency_penalty attribute should exist"
            );
            assert!(
                get_span_attribute(llm, "gen_ai.request.presence_penalty").is_some(),
                "presence_penalty attribute should exist"
            );
        }

        #[tokio::test]
        async fn test_span_response_attributes_recorded() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::OpenAI, false);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline = create_test_pipeline(vec!["test-model"]);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::chat::ChatCompletionRequest;
            use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
            let request_body = ChatCompletionRequest {
                model: "test".to_string(),
                messages: vec![ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                }],
                temperature: None,
                top_p: None,
                n: None,
                stream: None,
                stop: None,
                max_tokens: None,
                max_completion_tokens: None,
                presence_penalty: None,
                frequency_penalty: None,
                logit_bias: None,
                user: None,
                response_format: None,
                tools: None,
                tool_choice: None,
                parallel_tool_calls: None,
                logprobs: None,
                top_logprobs: None,
                reasoning: None,
            };

            let _response = app
                .oneshot(
                    Request::builder()
                        .uri("/chat/completions")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            let spans = get_spans_for_test(before_count);
            let llm = get_llm_span(&spans, "chat").expect("LLM span not found");

            // Verify response attributes
            assert_span_attributes(
                llm,
                &[
                    ("gen_ai.response.model", "test"),
                    ("gen_ai.response.id", "chatcmpl-test123"),
                ],
            );

            // Verify usage tokens
            assert_eq!(
                get_span_attribute(llm, "gen_ai.usage.prompt_tokens"),
                Some("10".to_string())
            );
            assert_eq!(
                get_span_attribute(llm, "gen_ai.usage.completion_tokens"),
                Some("15".to_string())
            );
            assert_eq!(
                get_span_attribute(llm, "gen_ai.usage.total_tokens"),
                Some("25".to_string())
            );
        }

        #[tokio::test]
        async fn test_vendor_attribute_mapping() {
            let _ = &*TEST_EXPORTER;

            // Test each provider type maps to correct vendor name
            // Note: Skip VertexAI as it requires project_id and location params
            let test_cases = vec![
                (ProviderType::OpenAI, "openai"),
                (ProviderType::Anthropic, "Anthropic"),
                (ProviderType::Azure, "Azure"),
                (ProviderType::Bedrock, "AWS"),
            ];

            for (provider_type, expected_vendor) in test_cases {
                let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

                let provider_registry =
                    create_test_provider_registry_for_spans(provider_type.clone(), false);
                let model_configs = create_model_configs(vec!["test-model"]);
                let model_registry =
                    Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
                let pipeline = create_test_pipeline(vec!["test-model"]);
                let app = create_pipeline(&pipeline, &model_registry, None);

                use crate::models::chat::ChatCompletionRequest;
                use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
                let request_body = ChatCompletionRequest {
                    model: "test".to_string(),
                    messages: vec![ChatCompletionMessage {
                        role: "user".to_string(),
                        content: Some(ChatMessageContent::String("Test".to_string())),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                        refusal: None,
                    }],
                    temperature: None,
                    top_p: None,
                    n: None,
                    stream: None,
                    stop: None,
                    max_tokens: None,
                    max_completion_tokens: None,
                    presence_penalty: None,
                    frequency_penalty: None,
                    logit_bias: None,
                    user: None,
                    response_format: None,
                    tools: None,
                    tool_choice: None,
                    parallel_tool_calls: None,
                    logprobs: None,
                    top_logprobs: None,
                    reasoning: None,
                };

                let _response = app
                    .oneshot(
                        Request::builder()
                            .uri("/chat/completions")
                            .method("POST")
                            .header("content-type", "application/json")
                            .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();

                let spans = get_spans_for_test(before_count);
                let llm = get_llm_span(&spans, "chat")
                    .unwrap_or_else(|| panic!("LLM span not found for {:?}", provider_type));

                assert_eq!(
                    get_span_attribute(llm, "gen_ai.system").as_deref(),
                    Some(expected_vendor),
                    "Vendor attribute mismatch for {:?}",
                    provider_type
                );
            }
        }

        #[tokio::test]
        async fn test_span_parent_child_relationship() {
            let _ = &*TEST_EXPORTER;
            let before_count = TEST_EXPORTER.get_finished_spans().unwrap().len();

            let provider_registry =
                create_test_provider_registry_for_spans(ProviderType::OpenAI, false);
            let model_configs = create_model_configs(vec!["test-model"]);
            let model_registry =
                Arc::new(ModelRegistry::new(&model_configs, provider_registry).unwrap());
            let pipeline = create_test_pipeline(vec!["test-model"]);
            let app = create_pipeline(&pipeline, &model_registry, None);

            use crate::models::chat::ChatCompletionRequest;
            use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
            let request_body = ChatCompletionRequest {
                model: "test".to_string(),
                messages: vec![ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Test".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                }],
                temperature: None,
                top_p: None,
                n: None,
                stream: None,
                stop: None,
                max_tokens: None,
                max_completion_tokens: None,
                presence_penalty: None,
                frequency_penalty: None,
                logit_bias: None,
                user: None,
                response_format: None,
                tools: None,
                tool_choice: None,
                parallel_tool_calls: None,
                logprobs: None,
                top_logprobs: None,
                reasoning: None,
            };

            let _response = app
                .oneshot(
                    Request::builder()
                        .uri("/chat/completions")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            let spans = get_spans_for_test(before_count);
            let root = get_root_span(&spans).expect("Root span not found");
            let llm = get_llm_span(&spans, "chat").expect("LLM span not found");

            // Verify parent-child relationship
            assert_eq!(
                llm.parent_span_id,
                root.span_context.span_id(),
                "LLM span's parent_span_id should equal root span's span_id"
            );

            // Verify trace_id consistency
            assert_eq!(
                llm.span_context.trace_id(),
                root.span_context.trace_id(),
                "Both spans should share the same trace_id"
            );

            // Verify root span has no parent (is actually root)
            assert_eq!(
                root.parent_span_id.to_string(),
                "0000000000000000",
                "Root span should not have a parent"
            );
        }
    }
}
