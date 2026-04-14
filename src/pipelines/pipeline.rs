use crate::config::models::PipelineType;
use crate::models::chat::ChatCompletionResponse;
use crate::models::completion::CompletionRequest;
use crate::models::embeddings::EmbeddingsRequest;
use crate::models::streaming::ChatCompletionChunk;
use crate::pipelines::otel::OtelTracer;
use crate::providers::provider::get_vendor_name;
use crate::types::ProviderType;
use crate::{
    ai_models::registry::ModelRegistry,
    config::models::{Pipeline, PluginConfig},
    models::chat::ChatCompletionRequest,
};
use async_stream::stream;
use axum::http::{HeaderName, HeaderValue};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{IntoResponse, Sse};
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

const HEADER_PROVIDER: HeaderName = HeaderName::from_static("x-traceloop-provider");

fn inject_provider_header(response: &mut axum::response::Response, provider_type: &ProviderType) {
    if let Ok(value) = HeaderValue::from_str(&provider_type.to_string()) {
        response
            .headers_mut()
            .insert(HEADER_PROVIDER.clone(), value);
    }
}

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
            // Set vendor now that we know which model/provider we're using
            tracer.set_vendor(&get_vendor_name(&model.provider.r#type()));

            let response = model
                .chat_completions(payload.clone())
                .await
                .inspect_err(|e| {
                    eprintln!("Chat completion error for model {model_key}: {e:?}");
                })?;

            let provider_type = model.provider.r#type();

            if let ChatCompletionResponse::NonStream(completion) = response {
                tracer.log_success(&completion);
                let mut resp = Json(completion).into_response();
                inject_provider_header(&mut resp, &provider_type);
                return Ok(resp);
            }

            if let ChatCompletionResponse::Stream(stream) = response {
                let mut resp = Sse::new(trace_and_stream(tracer, stream))
                    .keep_alive(KeepAlive::default())
                    .into_response();
                inject_provider_header(&mut resp, &provider_type);
                return Ok(resp);
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
            // Set vendor now that we know which model/provider we're using
            tracer.set_vendor(&get_vendor_name(&model.provider.r#type()));

            let response = model.completions(payload.clone()).await.inspect_err(|e| {
                eprintln!("Completion error for model {model_key}: {e:?}");
            })?;
            tracer.log_success(&response);
            let mut resp = Json(response).into_response();
            inject_provider_header(&mut resp, &model.provider.r#type());
            return Ok(resp);
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
            let mut resp = Json(response).into_response();
            inject_provider_header(&mut resp, &model.provider.r#type());
            return Ok(resp);
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

    #[test]
    fn test_vendor_mapping_integration() {
        assert_eq!(get_vendor_name(&ProviderType::OpenAI), "openai");
        assert_eq!(get_vendor_name(&ProviderType::Anthropic), "Anthropic");
        assert_eq!(get_vendor_name(&ProviderType::Azure), "Azure");
        assert_eq!(get_vendor_name(&ProviderType::Bedrock), "AWS");
        assert_eq!(get_vendor_name(&ProviderType::VertexAI), "Google");
    }

    // ── Configurable mock provider for header tests ──────────────────────

    #[derive(Clone)]
    struct ConfigurableMockProvider {
        key: String,
        provider_type: ProviderType,
    }

    #[async_trait]
    impl Provider for ConfigurableMockProvider {
        fn new(_config: &ProviderConfig) -> Self {
            unimplemented!("Use struct literal instead")
        }

        fn key(&self) -> String {
            self.key.clone()
        }

        fn r#type(&self) -> ProviderType {
            self.provider_type
        }

        async fn chat_completions(
            &self,
            _payload: crate::models::chat::ChatCompletionRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::chat::ChatCompletionResponse, StatusCode> {
            Ok(crate::models::chat::ChatCompletionResponse::NonStream(
                crate::models::chat::ChatCompletion {
                    id: "test-id".to_string(),
                    object: Some("chat.completion".to_string()),
                    created: Some(1700000000),
                    model: "resolved-model".to_string(),
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
            Ok(crate::models::completion::CompletionResponse {
                id: "test-id".to_string(),
                object: "text_completion".to_string(),
                created: 1700000000,
                model: "resolved-model".to_string(),
                choices: vec![],
                usage: crate::models::usage::Usage::default(),
            })
        }

        async fn embeddings(
            &self,
            _payload: EmbeddingsRequest,
            _model_config: &ModelConfig,
        ) -> Result<crate::models::embeddings::EmbeddingsResponse, StatusCode> {
            Ok(crate::models::embeddings::EmbeddingsResponse {
                object: "list".to_string(),
                data: vec![],
                model: "resolved-model".to_string(),
                usage: crate::models::usage::EmbeddingUsage::default(),
            })
        }
    }

    /// Build a pipeline router backed by a single ConfigurableMockProvider.
    fn build_mock_pipeline(
        provider_type: ProviderType,
        model: &str,
        pipeline_type: PipelineType,
    ) -> Router {
        let provider = Arc::new(ConfigurableMockProvider {
            key: "mock-provider".to_string(),
            provider_type,
        }) as Arc<dyn Provider>;

        let provider_registry = ProviderRegistry::from_mock("mock-provider".to_string(), provider);

        let model_configs = vec![ModelConfig {
            key: "mock-model".to_string(),
            r#type: model.to_string(),
            provider: "mock-provider".to_string(),
            params: HashMap::new(),
        }];

        let model_registry =
            ModelRegistry::new(&model_configs, Arc::new(provider_registry)).unwrap();

        let pipeline = Pipeline {
            name: "test".to_string(),
            r#type: pipeline_type,
            plugins: vec![PluginConfig::ModelRouter {
                models: vec!["mock-model".to_string()],
            }],
        };

        create_pipeline(&pipeline, &model_registry)
    }

    fn chat_request_body(model: &str) -> String {
        serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "hello"}]
        })
        .to_string()
    }

    fn completion_request_body(model: &str) -> String {
        serde_json::json!({
            "model": model,
            "prompt": "hello"
        })
        .to_string()
    }

    fn embedding_request_body(model: &str) -> String {
        serde_json::json!({
            "model": model,
            "input": "hello"
        })
        .to_string()
    }

    // ── Provider header tests: chat completions ─────────────────────────

    async fn assert_chat_provider_header(
        provider_type: ProviderType,
        model: &str,
        expected_provider: &str,
    ) {
        let app = build_mock_pipeline(provider_type, model, PipelineType::Chat);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/chat/completions")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(chat_request_body(model)))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-traceloop-provider").unwrap(),
            expected_provider,
            "X-Traceloop-Provider mismatch for {provider_type}"
        );
    }

    #[tokio::test]
    async fn test_chat_headers_openai_gpt4o() {
        assert_chat_provider_header(ProviderType::OpenAI, "gpt-4o", "openai").await;
    }

    #[tokio::test]
    async fn test_chat_headers_openai_gpt4o_mini() {
        assert_chat_provider_header(ProviderType::OpenAI, "gpt-4o-mini", "openai").await;
    }

    #[tokio::test]
    async fn test_chat_headers_openai_o1() {
        assert_chat_provider_header(ProviderType::OpenAI, "o1", "openai").await;
    }

    #[tokio::test]
    async fn test_chat_headers_openai_o3() {
        assert_chat_provider_header(ProviderType::OpenAI, "o3", "openai").await;
    }

    #[tokio::test]
    async fn test_chat_headers_openai_o3_mini() {
        assert_chat_provider_header(ProviderType::OpenAI, "o3-mini", "openai").await;
    }

    #[tokio::test]
    async fn test_chat_headers_anthropic_claude_sonnet() {
        assert_chat_provider_header(
            ProviderType::Anthropic,
            "claude-sonnet-4-20250514",
            "anthropic",
        )
        .await;
    }

    #[tokio::test]
    async fn test_chat_headers_anthropic_claude_haiku() {
        assert_chat_provider_header(
            ProviderType::Anthropic,
            "claude-haiku-4-5-20251001",
            "anthropic",
        )
        .await;
    }

    #[tokio::test]
    async fn test_chat_headers_azure_gpt4o() {
        assert_chat_provider_header(ProviderType::Azure, "gpt-4o", "azure").await;
    }

    #[tokio::test]
    async fn test_chat_headers_bedrock_claude() {
        assert_chat_provider_header(
            ProviderType::Bedrock,
            "anthropic.claude-sonnet-4-20250514-v1:0",
            "bedrock",
        )
        .await;
    }

    #[tokio::test]
    async fn test_chat_headers_vertexai_gemini() {
        assert_chat_provider_header(ProviderType::VertexAI, "gemini-2.5-pro", "vertexai").await;
    }

    // ── Provider header tests: completions ──────────────────────────────

    async fn assert_completion_provider_header(
        provider_type: ProviderType,
        model: &str,
        expected_provider: &str,
    ) {
        let app = build_mock_pipeline(provider_type, model, PipelineType::Completion);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/completions")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(completion_request_body(model)))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-traceloop-provider").unwrap(),
            expected_provider,
        );
    }

    #[tokio::test]
    async fn test_completion_headers_openai() {
        assert_completion_provider_header(ProviderType::OpenAI, "gpt-3.5-turbo-instruct", "openai")
            .await;
    }

    #[tokio::test]
    async fn test_completion_headers_azure() {
        assert_completion_provider_header(ProviderType::Azure, "gpt-35-turbo-instruct", "azure")
            .await;
    }

    // ── Provider header tests: embeddings ───────────────────────────────

    async fn assert_embedding_provider_header(
        provider_type: ProviderType,
        model: &str,
        expected_provider: &str,
    ) {
        let app = build_mock_pipeline(provider_type, model, PipelineType::Embeddings);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/embeddings")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(embedding_request_body(model)))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-traceloop-provider").unwrap(),
            expected_provider,
        );
    }

    #[tokio::test]
    async fn test_embedding_headers_openai_text_embedding_3_large() {
        assert_embedding_provider_header(ProviderType::OpenAI, "text-embedding-3-large", "openai")
            .await;
    }

    #[tokio::test]
    async fn test_embedding_headers_openai_text_embedding_3_small() {
        assert_embedding_provider_header(ProviderType::OpenAI, "text-embedding-3-small", "openai")
            .await;
    }

    #[tokio::test]
    async fn test_embedding_headers_azure() {
        assert_embedding_provider_header(ProviderType::Azure, "text-embedding-ada-002", "azure")
            .await;
    }

    #[tokio::test]
    async fn test_embedding_headers_bedrock_titan() {
        assert_embedding_provider_header(
            ProviderType::Bedrock,
            "amazon.titan-embed-text-v2:0",
            "bedrock",
        )
        .await;
    }

    #[tokio::test]
    async fn test_embedding_headers_vertexai() {
        assert_embedding_provider_header(ProviderType::VertexAI, "text-embedding-005", "vertexai")
            .await;
    }

    // ── ProviderType Display (used for header serialization) ────────────

    #[test]
    fn test_provider_type_display_all_variants() {
        assert_eq!(ProviderType::OpenAI.to_string(), "openai");
        assert_eq!(ProviderType::Anthropic.to_string(), "anthropic");
        assert_eq!(ProviderType::Azure.to_string(), "azure");
        assert_eq!(ProviderType::Bedrock.to_string(), "bedrock");
        assert_eq!(ProviderType::VertexAI.to_string(), "vertexai");
    }

    // ── No-match returns 404 without header ─────────────────────────────

    #[tokio::test]
    async fn test_chat_no_match_returns_404() {
        let app = build_mock_pipeline(ProviderType::OpenAI, "gpt-4o", PipelineType::Chat);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/chat/completions")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(chat_request_body("nonexistent-model")))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(response.headers().get("x-traceloop-provider").is_none());
    }
}
