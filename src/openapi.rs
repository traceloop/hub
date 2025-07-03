use utoipa::OpenApi;

// Import OSS models that need to be documented
use crate::models::{
    chat::{ChatCompletion, ChatCompletionRequest},
    completion::{CompletionRequest, CompletionResponse},
    embeddings::{EmbeddingsRequest, EmbeddingsResponse},
    streaming::ChatCompletionChunk,
};

// Conditionally import EE types when feature is enabled
#[cfg(feature = "db_based_config")]
use ee::{
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

/// Base OpenAPI documentation for OSS features
#[derive(OpenApi)]
#[openapi(
    paths(
        // OSS health and metrics endpoints
        health_handler,
        metrics_handler,
        // OSS pipeline endpoints (dynamically generated)
        chat_completions_handler,
        completions_handler,
        embeddings_handler,
    ),
    components(
        schemas(
            // OSS request/response models
            ChatCompletionRequest,
            ChatCompletion,
            ChatCompletionChunk,
            CompletionRequest,
            CompletionResponse,
            EmbeddingsRequest,
            EmbeddingsResponse,
        )
    ),
    tags(
        (name = "Health", description = "Health check and monitoring endpoints"),
        (name = "Chat", description = "Chat completion endpoints"),
        (name = "Completions", description = "Text completion endpoints"),
        (name = "Embeddings", description = "Text embedding endpoints"),
    ),
    info(
        title = "Hub LLM Gateway API",
        version = "1.0.0",
        description = "Hub LLM Gateway - Open Source LLM Gateway with optional Enterprise Edition features",
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
        (url = "/", description = "Local server")
    )
)]
pub struct OssApiDoc;

/// Extended OpenAPI documentation that includes EE features when available
#[cfg(feature = "db_based_config")]
#[derive(OpenApi)]
#[openapi(
    paths(
        // OSS endpoints
        health_handler,
        metrics_handler,
        chat_completions_handler,
        completions_handler,
        embeddings_handler,
        // EE Management API endpoints
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
            // OSS models
            ChatCompletionRequest,
            ChatCompletion,
            ChatCompletionChunk,
            CompletionRequest,
            CompletionResponse,
            EmbeddingsRequest,
            EmbeddingsResponse,
            // EE models
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
        (name = "Providers", description = "Provider management endpoints (Enterprise Edition)"),
        (name = "Model Definitions", description = "Model definition management endpoints (Enterprise Edition)"),
        (name = "Pipelines", description = "Pipeline management endpoints (Enterprise Edition)"),
    ),
    info(
        title = "Hub LLM Gateway API (Enterprise Edition)",
        version = "1.0.0",
        description = "Hub LLM Gateway with Enterprise Edition Management API - Complete LLM Gateway with configuration management",
        contact(
            name = "Traceloop",
            url = "https://traceloop.com",
            email = "support@traceloop.com"
        ),
        license(
            name = "Commercial",
            url = "https://traceloop.com/license"
        )
    ),
    servers(
        (url = "/", description = "Local server")
    )
)]
pub struct DbBasedApiDoc;

/// Get the appropriate OpenAPI documentation based on feature flags
pub fn get_openapi_spec() -> utoipa::openapi::OpenApi {
    #[cfg(feature = "db_based_config")]
    {
        DbBasedApiDoc::openapi()
    }
    #[cfg(not(feature = "db_based_config"))]
    {
        OssApiDoc::openapi()
    }
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
pub async fn metrics_handler() -> String {
    "# Prometheus metrics".to_string()
}

#[utoipa::path(
    post,
    path = "/api/v1/chat/completions",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "Chat completion response", body = ChatCompletion),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Chat"
)]
pub async fn chat_completions_handler() {}

#[utoipa::path(
    post,
    path = "/api/v1/completions",
    request_body = CompletionRequest,
    responses(
        (status = 200, description = "Text completion response", body = CompletionResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error"),
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
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Embeddings"
)]
pub async fn embeddings_handler() {}
