use utoipa::OpenApi;

// Import OSS models that need to be documented
use crate::models::{
    chat::{ChatCompletion, ChatCompletionRequest},
    completion::{CompletionRequest, CompletionResponse},
    embeddings::{EmbeddingsRequest, EmbeddingsResponse},
    streaming::ChatCompletionChunk,
};

// Always import management API components
use crate::management::{
    api::routes::{model_definition_routes::*, pipeline_routes::*, provider_routes::*},
    dto::{
        AnthropicProviderConfig, AzureProviderConfig, BedrockProviderConfig,
        CreateModelDefinitionRequest, CreatePipelineRequestDto, CreateProviderRequest,
        ModelDefinitionResponse, ModelRouterConfigDto, ModelRouterModelEntryDto,
        ModelRouterStrategyDto, OpenAIProviderConfig, PipelinePluginConfigDto, PipelineResponseDto,
        PluginType, ProviderConfig, ProviderResponse, ProviderType, UpdateModelDefinitionRequest,
        UpdatePipelineRequestDto, UpdateProviderRequest, VertexAIProviderConfig,
    },
    errors::ApiError,
};

/// Unified OpenAPI documentation for Traceloop Hub Gateway
/// Includes both core LLM gateway features and management API
#[derive(OpenApi)]
#[openapi(
    paths(
        // Core LLM Gateway endpoints
        health_handler,
        metrics_handler,
        chat_completions_handler,
        completions_handler,
        embeddings_handler,
        // Management API endpoints (available in database mode only)
        create_provider_handler,
        list_providers_handler,
        get_provider_handler,
        update_provider_handler,
        delete_provider_handler,
        create_model_definition_handler,
        list_model_definitions_handler,
        get_model_definition_handler,
        get_model_definition_by_key_handler,
        update_model_definition_handler,
        delete_model_definition_handler,
        create_pipeline_handler,
        list_pipelines_handler,
        get_pipeline_handler,
        get_pipeline_by_name_handler,
        update_pipeline_handler,
        delete_pipeline_handler,
    ),
    components(
        schemas(
            // Core LLM Gateway models
            ChatCompletionRequest,
            ChatCompletion,
            ChatCompletionChunk,
            CompletionRequest,
            CompletionResponse,
            EmbeddingsRequest,
            EmbeddingsResponse,
            // Management API models
            ApiError,
            ProviderType,
            ProviderConfig,
            OpenAIProviderConfig,
            AnthropicProviderConfig,
            AzureProviderConfig,
            BedrockProviderConfig,
            VertexAIProviderConfig,
            CreateProviderRequest,
            UpdateProviderRequest,
            ProviderResponse,
            CreateModelDefinitionRequest,
            UpdateModelDefinitionRequest,
            ModelDefinitionResponse,
            CreatePipelineRequestDto,
            UpdatePipelineRequestDto,
            PipelineResponseDto,
            PipelinePluginConfigDto,
            PluginType,
            ModelRouterConfigDto,
            ModelRouterModelEntryDto,
            ModelRouterStrategyDto,
        )
    ),
    tags(
        (name = "Health", description = "Health check and monitoring endpoints"),
        (name = "Chat", description = "Chat completion endpoints"),
        (name = "Completions", description = "Text completion endpoints"),
        (name = "Embeddings", description = "Text embedding endpoints"),
        (name = "Providers", description = "Provider management endpoints (database mode only)"),
        (name = "Model Definitions", description = "Model definition management endpoints (database mode only)"),
        (name = "Pipelines", description = "Pipeline management endpoints (database mode only)"),
    ),
    info(
        title = "Traceloop Hub LLM Gateway API",
        version = "1.0.0",
        description = "Traceloop Hub LLM Gateway - Open Source LLM Gateway with YAML and Database configuration modes. Management API endpoints are only available when running in database mode.",
        contact(
            name = "Traceloop",
            url = "https://traceloop.com",
            email = "support@traceloop.com"
        ),
        license(
            name = "Apache 2.0",
            url = "https://www.apache.org/licenses/LICENSE-2.0"
        )
    ),
    servers(
        (url = "http://localhost:3000", description = "LLM Gateway Server"),
        (url = "http://localhost:8080", description = "Management API Server")
    )
)]
pub struct HubApiDoc;

/// Get the OpenAPI specification
pub fn get_openapi_spec() -> utoipa::openapi::OpenApi {
    HubApiDoc::openapi()
}

// Placeholder handler functions for OpenAPI path documentation
// These will be replaced with actual handlers in the routes module

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = String),
    ),
    tag = "Health"
)]
pub async fn health_handler() -> &'static str {
    "Working!"
}

#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Prometheus metrics", body = String),
    ),
    tag = "Health"
)]
pub async fn metrics_handler() -> &'static str {
    "metrics"
}

#[utoipa::path(
    post,
    path = "/api/v1/chat/completions",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "Chat completion response", body = ChatCompletion),
    ),
    tag = "Chat"
)]
pub async fn chat_completions_handler() {}

#[utoipa::path(
    post,
    path = "/api/v1/completions",
    request_body = CompletionRequest,
    responses(
        (status = 200, description = "Completion response", body = CompletionResponse),
    ),
    tag = "Completions"
)]
pub async fn completions_handler() {}

#[utoipa::path(
    post,
    path = "/api/v1/embeddings",
    request_body = EmbeddingsRequest,
    responses(
        (status = 200, description = "Embeddings response", body = EmbeddingsResponse),
    ),
    tag = "Embeddings"
)]
pub async fn embeddings_handler() {}
