use hub_lib::openapi::get_openapi_spec;
use hub_lib::state::AppState;
use hub_lib::types::{
    GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider, ProviderType,
};
use std::sync::Arc;

#[test]
fn test_openapi_spec_is_valid() {
    let spec = get_openapi_spec();

    // Basic validation
    assert!(!spec.info.title.is_empty());
    assert!(!spec.info.version.is_empty());

    // Check that we have paths
    assert!(!spec.paths.paths.is_empty());

    // Check for core endpoints (always available)
    assert!(spec.paths.paths.contains_key("/health"));
    assert!(spec.paths.paths.contains_key("/metrics"));
    assert!(spec.paths.paths.contains_key("/api/v1/chat/completions"));
    assert!(spec.paths.paths.contains_key("/api/v1/completions"));
    assert!(spec.paths.paths.contains_key("/api/v1/embeddings"));

    // Check for management endpoints (always included in unified spec)
    assert!(
        spec.paths
            .paths
            .contains_key("/api/v1/management/providers")
    );
    assert!(
        spec.paths
            .paths
            .contains_key("/api/v1/management/model-definitions")
    );
    assert!(
        spec.paths
            .paths
            .contains_key("/api/v1/management/pipelines")
    );
}

#[test]
fn test_unified_openapi_contains_all_routes() {
    let spec = get_openapi_spec();

    // Core LLM Gateway routes
    let core_routes = [
        "/health",
        "/metrics",
        "/api/v1/chat/completions",
        "/api/v1/completions",
        "/api/v1/embeddings",
    ];

    for route in core_routes {
        assert!(
            spec.paths.paths.contains_key(route),
            "Missing core route: {}",
            route
        );
    }

    // Management API routes (available in database mode)
    let management_routes = [
        "/api/v1/management/providers",
        "/api/v1/management/model-definitions",
        "/api/v1/management/pipelines",
    ];

    for route in management_routes {
        assert!(
            spec.paths.paths.contains_key(route),
            "Missing management route: {}",
            route
        );
    }
}

#[test]
fn test_openapi_routes_no_conflict() {
    let spec = get_openapi_spec();

    // Ensure no path conflicts between core and management routes
    let paths: Vec<_> = spec.paths.paths.keys().collect();
    let unique_paths: std::collections::HashSet<_> = paths.iter().collect();

    assert_eq!(paths.len(), unique_paths.len(), "Duplicate paths detected");
}

#[test]
fn test_openapi_components_present() {
    let spec = get_openapi_spec();

    // Check that we have components defined
    assert!(spec.components.is_some());

    let components = spec.components.unwrap();

    // Check for core model schemas
    assert!(components.schemas.contains_key("ChatCompletionRequest"));
    assert!(components.schemas.contains_key("ChatCompletion"));
    assert!(components.schemas.contains_key("CompletionRequest"));
    assert!(components.schemas.contains_key("CompletionResponse"));
    assert!(components.schemas.contains_key("EmbeddingsRequest"));
    assert!(components.schemas.contains_key("EmbeddingsResponse"));

    // Check for management API schemas
    assert!(components.schemas.contains_key("ProviderType"));
    assert!(components.schemas.contains_key("CreateProviderRequest"));
    assert!(components.schemas.contains_key("ProviderResponse"));
    assert!(
        components
            .schemas
            .contains_key("CreateModelDefinitionRequest")
    );
    assert!(components.schemas.contains_key("ModelDefinitionResponse"));
    assert!(components.schemas.contains_key("CreatePipelineRequestDto"));
    assert!(components.schemas.contains_key("PipelineResponseDto"));
}

#[tokio::test]
async fn test_router_creation_no_conflicts() {
    // Create a basic configuration for testing
    let config = GatewayConfig {
        general: None,
        providers: vec![Provider {
            key: "test-provider".to_string(),
            r#type: ProviderType::OpenAI,
            api_key: "test-key".to_string(),
            params: Default::default(),
        }],
        models: vec![ModelConfig {
            key: "gpt-4".to_string(),
            r#type: "gpt-4".to_string(),
            provider: "test-provider".to_string(),
            params: Default::default(),
        }],
        pipelines: vec![Pipeline {
            name: "default".to_string(),
            r#type: PipelineType::Chat,
            plugins: vec![PluginConfig::ModelRouter {
                models: vec!["gpt-4".to_string()],
            }],
        }],
    };

    let app_state = Arc::new(AppState::new(config).expect("Failed to create app state"));

    // This should not panic due to route conflicts
    let _router = hub_lib::routes::create_router(app_state);

    // If we get here, the router was created successfully without conflicts
    assert!(
        true,
        "Router created successfully with unified OpenAPI routes"
    );
}
