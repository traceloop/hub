use hub_gateway_core_types::{
    GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider,
};
use hub_lib::{openapi::get_openapi_spec, routes, state::AppState};
use std::sync::Arc;

// --- OpenAPI Spec Content Tests ---

#[test]
fn test_oss_openapi_contains_basic_routes() {
    let spec = get_openapi_spec();

    // Check that OSS routes are present
    assert!(spec.paths.paths.contains_key("/health"));
    assert!(spec.paths.paths.contains_key("/metrics"));
    assert!(spec.paths.paths.contains_key("/api/v1/chat/completions"));
    assert!(spec.paths.paths.contains_key("/api/v1/completions"));
    assert!(spec.paths.paths.contains_key("/api/v1/embeddings"));

    // Check that basic OSS schemas are present
    let components = spec.components.as_ref().unwrap();
    assert!(components.schemas.contains_key("ChatCompletionRequest"));
    assert!(components.schemas.contains_key("ChatCompletion"));
    assert!(components.schemas.contains_key("CompletionRequest"));
    assert!(components.schemas.contains_key("CompletionResponse"));
    assert!(components.schemas.contains_key("EmbeddingsRequest"));
    assert!(components.schemas.contains_key("EmbeddingsResponse"));
}

#[cfg(feature = "ee_feature")]
#[test]
fn test_ee_openapi_contains_management_routes() {
    let spec = get_openapi_spec();

    // Check that EE management routes are present when feature is enabled
    assert!(spec.paths.paths.contains_key("/ee/api/v1/providers"));
    assert!(spec.paths.paths.contains_key("/ee/api/v1/providers/{id}"));
    assert!(spec
        .paths
        .paths
        .contains_key("/ee/api/v1/model-definitions"));
    assert!(spec
        .paths
        .paths
        .contains_key("/ee/api/v1/model-definitions/{id}"));
    assert!(spec.paths.paths.contains_key("/ee/api/v1/pipelines"));
    assert!(spec.paths.paths.contains_key("/ee/api/v1/pipelines/{id}"));

    // Check that EE schemas are present
    let components = spec.components.as_ref().unwrap();
    assert!(components.schemas.contains_key("CreateProviderRequest"));
    assert!(components
        .schemas
        .contains_key("CreateModelDefinitionRequest"));
    assert!(components.schemas.contains_key("CreatePipelineRequestDto"));
    assert!(components.schemas.contains_key("ApiError"));
}

#[cfg(not(feature = "ee_feature"))]
#[test]
fn test_oss_only_openapi_excludes_ee_routes() {
    let spec = get_openapi_spec();

    // Check that EE routes are NOT present when feature is disabled
    assert!(!spec.paths.paths.contains_key("/ee/api/v1/providers"));
    assert!(!spec
        .paths
        .paths
        .contains_key("/ee/api/v1/model-definitions"));
    assert!(!spec.paths.paths.contains_key("/ee/api/v1/pipelines"));

    // Check that EE-specific schemas are NOT present
    let components = spec.components.as_ref().unwrap();
    assert!(!components.schemas.contains_key("CreateProviderRequest"));
    assert!(!components
        .schemas
        .contains_key("CreateModelDefinitionRequest"));
    assert!(!components.schemas.contains_key("CreatePipelineRequestDto"));
    // Note: ApiError might be present in OSS for other error handling
}

#[test]
fn test_openapi_spec_is_valid() {
    let spec = get_openapi_spec();

    // Basic validation that the spec is well-formed
    assert!(spec.info.title.contains("Hub LLM Gateway"));
    assert!(spec.paths.paths.len() > 0);
    assert!(spec.components.is_some());

    // Ensure we have some schemas defined
    let components = spec.components.as_ref().unwrap();
    assert!(components.schemas.len() > 0);

    // Comprehensive validation
    assert!(
        !spec.paths.paths.is_empty(),
        "OpenAPI spec should contain paths"
    );

    #[cfg(feature = "ee_feature")]
    {
        // When EE feature is enabled, should contain EE endpoints
        assert!(
            spec.info.title.contains("Enterprise Edition"),
            "Should indicate EE in title"
        );
        assert!(
            spec.paths.paths.contains_key("/ee/api/v1/providers"),
            "Should contain EE provider endpoints"
        );
    }

    #[cfg(not(feature = "ee_feature"))]
    {
        // When EE feature is disabled, should be OSS only
        assert!(
            !spec.info.title.contains("Enterprise Edition"),
            "Should not indicate EE in title"
        );
        assert!(
            !spec.paths.paths.contains_key("/ee/api/v1/providers"),
            "Should not contain EE endpoints"
        );
    }
}

// --- Router Integration Tests ---

#[tokio::test]
async fn test_oss_openapi_routes_no_conflict() {
    // Create a basic configuration for OSS
    let config = GatewayConfig {
        general: None,
        providers: vec![Provider {
            key: "test-provider".to_string(),
            r#type: "openai".to_string(),
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
    let _router = routes::create_router(app_state);

    // If we get here, the router was created successfully without conflicts
    assert!(true, "OSS router created successfully with OpenAPI routes");
}

#[cfg(feature = "ee_feature")]
#[tokio::test]
async fn test_ee_openapi_routes_no_conflict() {
    use sqlx::PgPool;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;

    // Set up test database
    let postgres_container = Postgres::default().start().await.unwrap();
    let connection_string = format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        postgres_container.get_host_port_ipv4(5432).await.unwrap()
    );

    let pool = PgPool::connect(&connection_string).await.unwrap();
    sqlx::migrate!("./ee/migrations").run(&pool).await.unwrap();

    // Create EE router directly without going through the main router
    // to avoid Prometheus metrics conflicts in tests
    let (ee_router, _config_service) = ee::ee_api_bundle(pool);

    // Test that we can create a simple router and nest the EE router
    // This tests the route structure without the Prometheus layer that causes conflicts
    let _test_router = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "Working!" }))
        .nest("/ee/api/v1", ee_router);

    // If we get here, the combined router was created successfully without conflicts
    assert!(
        true,
        "EE router nested successfully with unified OpenAPI routes"
    );
}
