use crate::config::models::PipelineType;
use crate::guardrails::guardrails_runner::GuardrailsRunner;
use crate::guardrails::types::{GuardrailResources, Guardrails};
use crate::models::chat::ChatCompletionResponse;
use crate::models::completion::CompletionRequest;
use crate::models::embeddings::EmbeddingsRequest;
use crate::models::streaming::ChatCompletionChunk;
use crate::pipelines::otel::OtelTracer;
use crate::providers::provider::get_vendor_name;
use crate::{
    ai_models::registry::ModelRegistry,
    config::models::{Pipeline, PluginConfig},
    models::chat::ChatCompletionRequest,
};
use async_stream::stream;
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Response, Sse};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use reqwest_streams::error::StreamBodyError;
use std::sync::Arc;

// Re-export builder and orchestrator functions for backward compatibility with tests
pub use crate::guardrails::builder::{
    build_guardrail_resources, build_pipeline_guardrails, resolve_guard_defaults,
};
pub use crate::guardrails::guardrails_runner::{blocked_response, warning_header_value};

pub fn create_pipeline(
    pipeline: &Pipeline,
    model_registry: &ModelRegistry,
    guardrail_resources: Option<&GuardrailResources>,
) -> Router {
    let guardrails: Option<Arc<Guardrails>> = guardrail_resources
        .map(|shared| build_pipeline_guardrails(shared, &pipeline.guards));
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
        let gr = guardrails.clone();
        router = match plugin {
            PluginConfig::Tracing { endpoint, api_key } => {
                tracing::info!("Initializing OtelTracer for pipeline {}", pipeline.name);
                OtelTracer::init(endpoint, api_key);
                router
            }
            PluginConfig::ModelRouter { models } => match pipeline.r#type {
                PipelineType::Chat => {
                    router.route(
                        "/chat/completions",
                        post(move |state, headers, payload| chat_completions(state, headers, payload, models, gr)),
                    )
                }
                PipelineType::Completion => {
                    router.route(
                        "/completions",
                        post(move |state, payload| completions(state, payload, models)),
                    )
                }
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
    headers: HeaderMap,
    Json(payload): Json<ChatCompletionRequest>,
    model_keys: Vec<String>,
    guardrails: Option<Arc<Guardrails>>,
) -> Result<Response, StatusCode> {
    let mut tracer = OtelTracer::start("chat", &payload);
    let orchestrator = GuardrailsRunner::new(guardrails.as_deref(), &headers);

    // Pre-call guardrails
    let mut all_warnings = Vec::new();
    if let Some(orch) = &orchestrator {
        let pre = orch.run_pre_call(&payload).await;
        if let Some(resp) = pre.blocked_response {
            return Ok(resp);
        }
        all_warnings = pre.warnings;
    }

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            tracer.set_vendor(&get_vendor_name(&model.provider.r#type()));

            let response = model
                .chat_completions(payload.clone())
                .await
                .inspect_err(|e| {
                    eprintln!("Chat completion error for model {model_key}: {e:?}");
                })?;

            if let ChatCompletionResponse::NonStream(completion) = response {
                tracer.log_success(&completion);

                // Post-call guardrails (non-streaming)
                if let Some(orch) = &orchestrator {
                    let post = orch.run_post_call(&completion).await;
                    if let Some(resp) = post.blocked_response {
                        return Ok(resp);
                    }
                    all_warnings.extend(post.warnings);
                }

                return Ok(GuardrailsRunner::finalize_response(
                    Json(completion).into_response(),
                    &all_warnings,
                ));
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
) -> Result<Response, StatusCode> {
    let mut tracer = OtelTracer::start("completion", &payload);

    for model_key in model_keys {
        let model = model_registry.get(&model_key).unwrap();

        if payload.model == model.model_type {
            tracer.set_vendor(&get_vendor_name(&model.provider.r#type()));

            let response = model.completions(payload.clone()).await.inspect_err(|e| {
                eprintln!("Completion error for model {model_key}: {e:?}");
            })?;
            tracer.log_success(&response);

            return Ok(Json(response).into_response());
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
            // Set vendor now that we know which model/provider we're using
            tracer.set_vendor(&get_vendor_name(&model.provider.r#type()));

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
}
