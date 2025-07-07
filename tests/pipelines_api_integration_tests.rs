#![allow(dead_code)] // Allow dead code for work-in-progress
#![allow(unused_imports)] // Allow unused imports for work-in-progress

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use axum_test::TestServer;
use chrono::{DateTime, Utc};
use hub_lib::management::{
    api::routes::{model_definition_routes, pipeline_routes, provider_routes},
    db::models::{ModelDefinition, Pipeline, Provider},
    dto::{
        AnthropicProviderConfig, AzureProviderConfig, BedrockProviderConfig,
        CreateModelDefinitionRequest, CreatePipelineRequestDto, CreateProviderRequest,
        LoggingConfigDto, ModelDefinitionResponse, ModelRouterConfigDto, ModelRouterModelEntryDto,
        ModelRouterStrategyDto, OpenAIProviderConfig, PipelinePluginConfigDto, PipelineResponseDto,
        PluginType, ProviderConfig, ProviderResponse, ProviderType, SecretObject, TracingConfigDto,
        UpdatePipelineRequestDto, VertexAIProviderConfig,
    },
    errors::ApiError,
    management_api_bundle, AppState,
};
use serde_json::json;
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, types::Uuid, PgPool};
use std::path::Path;
use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;

async fn setup_test_environment() -> (TestServer, PgPool, impl Drop) {
    let node = Postgres::default()
        .with_user("testuser")
        .with_password("testpass")
        .with_db_name("testdb");

    let container = node
        .start()
        .await
        .expect("Failed to start Postgres container");

    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get container port");
    let db_url = format!("postgres://testuser:testpass@localhost:{}/testdb", port);
    println!("Test Postgres running at: {}", db_url);

    let pool = PgPoolOptions::new()
        .max_connections(5) // Reduce max connections per test
        .min_connections(1) // Ensure minimum connections
        .acquire_timeout(std::time::Duration::from_secs(30)) // Increase timeout
        .idle_timeout(std::time::Duration::from_secs(10)) // Cleanup idle connections
        .max_lifetime(std::time::Duration::from_secs(300)) // Recycle connections
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let migrations_path = Path::new(&manifest_dir).join("migrations");

    Migrator::new(migrations_path)
        .await
        .expect("Failed to load migrations from path")
        .run(&pool)
        .await
        .expect("Failed to run migrations on test DB");

    let (router, _config_provider) = management_api_bundle(pool.clone());
    let client = TestServer::new(router).expect("Failed to create TestServer");

    (client, pool, container) // Return Arc<PgPool> and container
}

// --- Test Cases Start Here ---

#[tokio::test]
async fn test_health_check() {
    let (server, _pool, _container) = setup_test_environment().await;
    let response = server.get("/health").await;
    response.assert_status_ok();
    response.assert_text("Management API is healthy");
}

// Placeholder for Pipeline Tests
// We will need to create providers and model definitions as prerequisites for many pipeline tests.

// Example of how to create a provider (if needed for setup, adapt as necessary)
async fn create_test_provider(
    server: &TestServer,
    key_suffix: &str,
    provider_type_enum: ProviderType,
) -> ProviderResponse {
    let name = format!("Test Provider {}", key_suffix);
    let provider_config = match provider_type_enum {
        ProviderType::OpenAI => ProviderConfig::OpenAI(OpenAIProviderConfig {
            api_key: SecretObject::literal(format!("openai_key_{}", key_suffix)),
            organization_id: Some(format!("openai_org_{}", key_suffix)),
        }),
        ProviderType::Azure => ProviderConfig::Azure(AzureProviderConfig {
            api_key: SecretObject::literal(format!("azure_key_{}", key_suffix)),
            api_version: "2023-05-15".to_string(),
            resource_name: format!("azure_res_{}", key_suffix),
            base_url: None,
        }),
        ProviderType::Anthropic => ProviderConfig::Anthropic(AnthropicProviderConfig {
            api_key: SecretObject::literal(format!("anthropic_key_{}", key_suffix)),
        }),
        ProviderType::Bedrock => ProviderConfig::Bedrock(BedrockProviderConfig {
            region: "us-east-1".to_string(),
            aws_access_key_id: Some(SecretObject::literal(format!(
                "bedrock_access_{}",
                key_suffix
            ))),
            aws_secret_access_key: Some(SecretObject::literal(format!(
                "bedrock_secret_{}",
                key_suffix
            ))),
            aws_session_token: None,
            use_iam_role: Some(false),
            inference_profile_id: None,
        }),
        ProviderType::VertexAI => ProviderConfig::VertexAI(VertexAIProviderConfig {
            project_id: format!("vertexai_project_{}", key_suffix),
            location: "us-central1".to_string(),
            credentials_path: Some(format!("/path/to/vertexai_creds_{}.json", key_suffix)),
            api_key: None,
        }),
    };

    let request_payload = json!({
        "name": name,
        "provider_type": provider_type_enum,
        "config": provider_config,
        "enabled": true
    });

    let response = server
        .post("/api/v1/management/providers")
        .json(&request_payload)
        .await;
    response.assert_status(StatusCode::CREATED);
    response.json()
}

// Example of how to create a model definition
async fn create_test_model_definition(
    server: &TestServer,
    provider_id: Uuid,
    key: &str,
) -> ModelDefinitionResponse {
    let request = CreateModelDefinitionRequest {
        key: key.to_string(),
        model_type: "gpt-4o".to_string(),
        provider_id,
        config_details: Some(json!({"temperature": 0.7})),
        enabled: Some(true),
    };
    let response = server
        .post("/api/v1/management/model-definitions")
        .json(&request)
        .await;
    response.assert_status(StatusCode::CREATED);
    response.json()
}

// --- Pipeline API Tests ---

#[tokio::test]
async fn test_create_pipeline_success_simple() {
    let (server, _pool, _container) = setup_test_environment().await;
    let pipeline_req = CreatePipelineRequestDto {
        name: "Test Simple Pipeline".to_string(),
        pipeline_type: "chat".to_string(),
        description: Some("A simple test pipeline".to_string()),
        plugins: vec![],
        enabled: true,
    };
    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = response.json();
    assert_eq!(created_pipeline.name, pipeline_req.name);
    assert!(created_pipeline.plugins.is_empty());
    let fetched_response = server
        .get(&format!(
            "/api/v1/management/pipelines/{}",
            created_pipeline.id
        ))
        .await;
    fetched_response.assert_status_ok();
    let fetched_pipeline: PipelineResponseDto = fetched_response.json();
    assert_eq!(fetched_pipeline.id, created_pipeline.id);
}

#[tokio::test]
async fn test_create_pipeline_name_conflict() {
    let (server, _pool, _container) = setup_test_environment().await;
    let pipeline_name = format!("Conflict Pipeline {}", Uuid::new_v4());
    let pipeline_req = CreatePipelineRequestDto {
        name: pipeline_name.clone(),
        pipeline_type: "chat".to_string(),
        description: None,
        plugins: vec![],
        enabled: true,
    };
    let creation_response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    creation_response.assert_status(StatusCode::CREATED);

    let conflict_response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    conflict_response.assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_get_pipeline_not_found() {
    let (server, _pool, _container) = setup_test_environment().await;
    let non_existent_id = Uuid::new_v4();
    server
        .get(&format!("/api/v1/management/pipelines/{}", non_existent_id))
        .await
        .assert_status_not_found();
}

#[tokio::test]
async fn test_create_pipeline_with_valid_model_router() {
    let (server, _pool, _container) = setup_test_environment().await;
    let provider = create_test_provider(
        &server,
        "openai-for-pipeline-valid-mr",
        ProviderType::OpenAI,
    )
    .await;
    let model_def_key = "gpt-4-for-pipeline-valid-mr";
    let model_def = create_test_model_definition(&server, provider.id, model_def_key).await;
    let pipeline_name = format!("Pipeline ValidMR {}", Uuid::new_v4());
    let plugin_config_data = json!({
        "strategy": "simple",
        "models": [
            { "key": model_def.key.clone(), "priority": 1, "weight": 100 }
        ]
    });
    let pipeline_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::ModelRouter,
        config_data: plugin_config_data,
        enabled: true,
        order_in_pipeline: 1,
    };
    let pipeline_req = CreatePipelineRequestDto {
        name: pipeline_name.clone(),
        pipeline_type: "llm-router".to_string(),
        description: Some("Test pipeline with a valid model router plugin".to_string()),
        plugins: vec![pipeline_plugin],
        enabled: true,
    };
    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = response.json();
    assert_eq!(created_pipeline.name, pipeline_name);
    assert_eq!(created_pipeline.plugins.len(), 1);
    let created_plugin = &created_pipeline.plugins[0];
    assert_eq!(created_plugin.plugin_type, PluginType::ModelRouter);
    let created_plugin_config: ModelRouterConfigDto =
        serde_json::from_value(created_plugin.config_data.clone())
            .expect("Failed to deserialize model-router config from response");
    assert_eq!(created_plugin_config.models.len(), 1);
    assert_eq!(created_plugin_config.models[0].key, model_def.key);
}

#[tokio::test]
async fn test_create_pipeline_with_invalid_model_router_key() {
    let (server, _pool, _container) = setup_test_environment().await;
    let non_existent_model_key = format!("non-existent-model-{}", Uuid::new_v4());
    let pipeline_name = format!("Pipeline InvalidMRKey {}", Uuid::new_v4());
    let plugin_config_data = json!({
        "strategy": "simple",
        "models": [
            { "key": non_existent_model_key.clone(), "priority": 1, "weight": 100 }
        ]
    });
    let pipeline_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::ModelRouter,
        config_data: plugin_config_data,
        enabled: true,
        order_in_pipeline: 1,
    };
    let pipeline_req = CreatePipelineRequestDto {
        name: pipeline_name.clone(),
        pipeline_type: "llm-router".to_string(),
        description: Some("Test pipeline with an invalid model router key".to_string()),
        plugins: vec![pipeline_plugin],
        enabled: true,
    };
    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
    let error_response: serde_json::Value = response.json();
    assert!(error_response.get("error").is_some());
    assert!(error_response
        .get("error")
        .unwrap()
        .as_str()
        .unwrap()
        .contains(&format!(
            "ModelDefinition key '{}' not found",
            non_existent_model_key
        )));
}

#[tokio::test]
async fn test_list_pipelines() {
    let (server, _pool, _container) = setup_test_environment().await;
    let pipeline_name1 = format!("Listable Pipeline 1 {}", Uuid::new_v4());
    let pipeline1_req = CreatePipelineRequestDto {
        name: pipeline_name1.clone(),
        pipeline_type: "chat".to_string(),
        description: None,
        plugins: vec![],
        enabled: true,
    };
    let response1 = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline1_req)
        .await;
    response1.assert_status(StatusCode::CREATED);
    let created_pipeline1: PipelineResponseDto = response1.json();

    let pipeline_name2 = format!("Listable Pipeline 2 {}", Uuid::new_v4());
    let pipeline2_req = CreatePipelineRequestDto {
        name: pipeline_name2.clone(),
        pipeline_type: "embeddings".to_string(),
        description: Some("Another listable".to_string()),
        plugins: vec![],
        enabled: false,
    };
    let response2 = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline2_req)
        .await;
    response2.assert_status(StatusCode::CREATED);
    let created_pipeline2: PipelineResponseDto = response2.json();

    let list_response = server.get("/api/v1/management/pipelines").await;
    list_response.assert_status_ok();
    let listed_pipelines: Vec<PipelineResponseDto> = list_response.json();
    assert!(listed_pipelines
        .iter()
        .any(|p| p.id == created_pipeline1.id && p.name == pipeline_name1));
    assert!(listed_pipelines
        .iter()
        .any(|p| p.id == created_pipeline2.id && p.name == pipeline_name2 && !p.enabled));
}

#[tokio::test]
async fn test_get_pipeline_by_name() {
    let (server, _pool, _container) = setup_test_environment().await;
    let pipeline_name = format!("Get-By-Name-Pipeline-{}", Uuid::new_v4());
    let pipeline_req = CreatePipelineRequestDto {
        name: pipeline_name.clone(),
        pipeline_type: "chat".to_string(),
        description: Some("Fetch by name".to_string()),
        plugins: vec![],
        enabled: true,
    };
    let creation_response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    creation_response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = creation_response.json();

    let fetch_response = server
        .get(&format!(
            "/api/v1/management/pipelines/name/{}",
            pipeline_name
        ))
        .await;
    fetch_response.assert_status_ok();
    let fetched_pipeline: PipelineResponseDto = fetch_response.json();
    assert_eq!(fetched_pipeline.id, created_pipeline.id);
    assert_eq!(fetched_pipeline.name, pipeline_name);
    let non_existent_name = format!("non-existent-name-{}", Uuid::new_v4());
    server
        .get(&format!(
            "/api/v1/management/pipelines/name/{}",
            non_existent_name
        ))
        .await
        .assert_status_not_found();
}

#[tokio::test]
async fn test_update_pipeline_name_and_plugins() {
    let (server, _pool, _container) = setup_test_environment().await;
    let provider =
        create_test_provider(&server, "openai-for-update-pipeline", ProviderType::OpenAI).await;
    let model_def1_key = "gpt-3.5-for-update-pipe";
    let model_def1 = create_test_model_definition(&server, provider.id, model_def1_key).await;
    let model_def2_key = "gpt-4-for-update-pipe";
    let model_def2 = create_test_model_definition(&server, provider.id, model_def2_key).await;
    let initial_pipeline_name = format!("Update Target Pipeline {}", Uuid::new_v4());
    let initial_plugin_config_data = json!({
        "strategy": "simple",
        "models": [{"key": model_def1.key.clone(), "priority": 1, "weight": 100}]
    });
    let initial_pipeline_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::ModelRouter,
        config_data: initial_plugin_config_data,
        enabled: true,
        order_in_pipeline: 1,
    };
    let create_req = CreatePipelineRequestDto {
        name: initial_pipeline_name.clone(),
        pipeline_type: "router-v1".to_string(),
        description: Some("Initial version".to_string()),
        plugins: vec![initial_pipeline_plugin],
        enabled: true,
    };
    let creation_response = server
        .post("/api/v1/management/pipelines")
        .json(&create_req)
        .await;
    creation_response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = creation_response.json();

    let updated_pipeline_name = format!("Updated Pipeline Name {}", Uuid::new_v4());
    let updated_plugin_config_data = json!({
        "strategy": "ordered_fallback",
        "models": [
            {"key": model_def1.key.clone(), "priority": 2, "weight": 50},
            {"key": model_def2.key.clone(), "priority": 1, "weight": 150}
        ]
    });
    let updated_model_router_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::ModelRouter,
        config_data: updated_plugin_config_data,
        enabled: false,
        order_in_pipeline: 1,
    };
    let new_simple_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Logging,
        config_data: json!({ "level": "strict"}),
        enabled: true,
        order_in_pipeline: 2,
    };
    let update_req = UpdatePipelineRequestDto {
        name: Some(updated_pipeline_name.clone()),
        pipeline_type: None,
        description: Some("Updated version".to_string()),
        plugins: Some(vec![updated_model_router_plugin, new_simple_plugin]),
        enabled: Some(false),
    };
    let update_response = server
        .put(&format!(
            "/api/v1/management/pipelines/{}",
            created_pipeline.id
        ))
        .json(&update_req)
        .await;
    update_response.assert_status_ok();
    let updated_pipeline: PipelineResponseDto = update_response.json();
    assert_eq!(updated_pipeline.id, created_pipeline.id);
    assert_eq!(updated_pipeline.name, updated_pipeline_name);
    assert_eq!(updated_pipeline.description, update_req.description);
    assert_eq!(updated_pipeline.enabled, update_req.enabled.unwrap());
    assert_eq!(updated_pipeline.plugins.len(), 2);
    let plugin1 = updated_pipeline
        .plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::ModelRouter)
        .unwrap();
    assert!(!plugin1.enabled);
    let plugin1_config: ModelRouterConfigDto =
        serde_json::from_value(plugin1.config_data.clone()).unwrap();
    assert_eq!(plugin1_config.models.len(), 2);
    let plugin2 = updated_pipeline
        .plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Logging)
        .unwrap();
    assert!(plugin2.enabled);
    assert_eq!(plugin2.order_in_pipeline, 2);
    assert_eq!(
        plugin2.config_data.get("level").unwrap().as_str().unwrap(),
        "strict"
    );
}

#[tokio::test]
async fn test_delete_pipeline() {
    let (server, _pool, _container) = setup_test_environment().await;
    let pipeline_name = format!("Delete Target Pipeline {}", Uuid::new_v4());
    let pipeline_req = CreatePipelineRequestDto {
        name: pipeline_name.clone(),
        pipeline_type: "temporary".to_string(),
        description: Some("To be deleted".to_string()),
        plugins: vec![],
        enabled: true,
    };
    let creation_response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    creation_response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = creation_response.json();

    let delete_response = server
        .delete(&format!(
            "/api/v1/management/pipelines/{}",
            created_pipeline.id
        ))
        .await;
    delete_response.assert_status_ok();
    server
        .get(&format!(
            "/api/v1/management/pipelines/{}",
            created_pipeline.id
        ))
        .await
        .assert_status_not_found();
    let non_existent_id = Uuid::new_v4();
    server
        .delete(&format!("/api/v1/management/pipelines/{}", non_existent_id))
        .await
        .assert_status_not_found();
}

#[tokio::test]
async fn test_create_pipeline_with_logging_plugin() {
    let (server, _pool, _container) = setup_test_environment().await;

    let logging_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Logging,
        config_data: json!({"level": "debug"}),
        enabled: true,
        order_in_pipeline: 1,
    };

    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Logging Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Pipeline with logging plugin".to_string()),
        plugins: vec![logging_plugin],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);

    let created_pipeline: PipelineResponseDto = response.json();
    assert_eq!(created_pipeline.name, pipeline_req.name);
    assert_eq!(created_pipeline.plugins.len(), 1);

    let logging_plugin = &created_pipeline.plugins[0];
    assert_eq!(logging_plugin.plugin_type, PluginType::Logging);
    assert!(logging_plugin.enabled);
    assert_eq!(logging_plugin.order_in_pipeline, 1);

    // Verify the config_data can be deserialized to LoggingConfigDto
    let logging_config: LoggingConfigDto =
        serde_json::from_value(logging_plugin.config_data.clone()).unwrap();
    assert_eq!(logging_config.level, "debug");
}

#[tokio::test]
async fn test_create_pipeline_with_tracing_plugin() {
    let (server, _pool, _container) = setup_test_environment().await;

    let tracing_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Tracing,
        config_data: json!({
            "endpoint": "http://trace.example.com/v1/traces",
            "api_key": {
                "type": "literal",
                "value": "test-api-key"
            }
        }),
        enabled: true,
        order_in_pipeline: 1,
    };

    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Tracing Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Pipeline with tracing plugin".to_string()),
        plugins: vec![tracing_plugin],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);

    let created_pipeline: PipelineResponseDto = response.json();
    assert_eq!(created_pipeline.name, pipeline_req.name);
    assert_eq!(created_pipeline.plugins.len(), 1);

    let tracing_plugin = &created_pipeline.plugins[0];
    assert_eq!(tracing_plugin.plugin_type, PluginType::Tracing);
    assert!(tracing_plugin.enabled);
    assert_eq!(tracing_plugin.order_in_pipeline, 1);

    // Verify the config_data can be deserialized to TracingConfigDto
    let tracing_config: TracingConfigDto =
        serde_json::from_value(tracing_plugin.config_data.clone()).unwrap();
    assert_eq!(
        tracing_config.endpoint,
        "http://trace.example.com/v1/traces"
    );
    assert_eq!(
        tracing_config.api_key,
        SecretObject::literal("test-api-key".to_string())
    );
}

#[tokio::test]
async fn test_create_pipeline_with_tracing_environment_secret() {
    let (server, _pool, _container) = setup_test_environment().await;

    let tracing_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Tracing,
        config_data: json!({
            "endpoint": "http://trace.example.com/v1/traces",
            "api_key": {
                "type": "environment",
                "variable_name": "TRACING_API_KEY"
            }
        }),
        enabled: true,
        order_in_pipeline: 1,
    };

    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Tracing Env Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Pipeline with tracing plugin using environment variable".to_string()),
        plugins: vec![tracing_plugin],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);

    let created_pipeline: PipelineResponseDto = response.json();
    let tracing_plugin = &created_pipeline.plugins[0];
    let tracing_config: TracingConfigDto =
        serde_json::from_value(tracing_plugin.config_data.clone()).unwrap();
    assert_eq!(
        tracing_config.api_key,
        SecretObject::environment("TRACING_API_KEY".to_string())
    );
}

#[tokio::test]
async fn test_create_pipeline_with_tracing_kubernetes_secret() {
    let (server, _pool, _container) = setup_test_environment().await;

    let tracing_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Tracing,
        config_data: json!({
            "endpoint": "http://trace.example.com/v1/traces",
            "api_key": {
                "type": "kubernetes",
                "secret_name": "tracing-secrets",
                "key": "api-key",
                "namespace": "monitoring"
            }
        }),
        enabled: true,
        order_in_pipeline: 1,
    };

    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Tracing K8s Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Pipeline with tracing plugin using Kubernetes secret".to_string()),
        plugins: vec![tracing_plugin],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);

    let created_pipeline: PipelineResponseDto = response.json();
    let tracing_plugin = &created_pipeline.plugins[0];
    let tracing_config: TracingConfigDto =
        serde_json::from_value(tracing_plugin.config_data.clone()).unwrap();
    assert_eq!(
        tracing_config.api_key,
        SecretObject::kubernetes(
            "tracing-secrets".to_string(),
            "api-key".to_string(),
            Some("monitoring".to_string())
        )
    );
}

#[tokio::test]
async fn test_create_pipeline_with_multiple_plugins() {
    let (server, _pool, _container) = setup_test_environment().await;

    let provider = create_test_provider(&server, "openai-multi-plugin", ProviderType::OpenAI).await;
    let model_def = create_test_model_definition(&server, provider.id, "gpt-4o-multi-plugin").await;

    let logging_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Logging,
        config_data: json!({"level": "info"}),
        enabled: true,
        order_in_pipeline: 1,
    };

    let tracing_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Tracing,
        config_data: json!({
            "endpoint": "http://trace.example.com/v1/traces",
            "api_key": {
                "type": "literal",
                "value": "multi-plugin-key"
            }
        }),
        enabled: true,
        order_in_pipeline: 2,
    };

    let model_router_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::ModelRouter,
        config_data: json!({
            "strategy": "simple",
            "models": [{"key": model_def.key, "priority": 1}]
        }),
        enabled: true,
        order_in_pipeline: 3,
    };

    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Multi Plugin Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Pipeline with multiple plugin types".to_string()),
        plugins: vec![logging_plugin, tracing_plugin, model_router_plugin],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);

    let created_pipeline: PipelineResponseDto = response.json();
    assert_eq!(created_pipeline.plugins.len(), 3);

    // Verify all plugins are present and correctly ordered
    let plugins = &created_pipeline.plugins;
    assert_eq!(plugins[0].plugin_type, PluginType::Logging);
    assert_eq!(plugins[0].order_in_pipeline, 1);
    assert_eq!(plugins[1].plugin_type, PluginType::Tracing);
    assert_eq!(plugins[1].order_in_pipeline, 2);
    assert_eq!(plugins[2].plugin_type, PluginType::ModelRouter);
    assert_eq!(plugins[2].order_in_pipeline, 3);
}

#[tokio::test]
async fn test_create_pipeline_with_invalid_logging_config() {
    let (server, _pool, _container) = setup_test_environment().await;

    let invalid_logging_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Logging,
        config_data: json!({"invalid_field": "value"}), // Missing required 'level' field
        enabled: true,
        order_in_pipeline: 1,
    };

    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Invalid Logging Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Pipeline with invalid logging config".to_string()),
        plugins: vec![invalid_logging_plugin],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_pipeline_with_invalid_tracing_config() {
    let (server, _pool, _container) = setup_test_environment().await;

    let invalid_tracing_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Tracing,
        config_data: json!({"endpoint": "http://trace.example.com"}), // Missing required 'api_key' field
        enabled: true,
        order_in_pipeline: 1,
    };

    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Invalid Tracing Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Pipeline with invalid tracing config".to_string()),
        plugins: vec![invalid_tracing_plugin],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_pipeline_with_logging_and_tracing() {
    let (server, _pool, _container) = setup_test_environment().await;

    // Create a simple pipeline first
    let pipeline_req = CreatePipelineRequestDto {
        name: format!("Update Target Pipeline {}", Uuid::new_v4()),
        pipeline_type: "chat".to_string(),
        description: Some("Initial pipeline".to_string()),
        plugins: vec![],
        enabled: true,
    };

    let response = server
        .post("/api/v1/management/pipelines")
        .json(&pipeline_req)
        .await;
    response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = response.json();

    // Update with logging and tracing plugins
    let logging_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Logging,
        config_data: json!({"level": "warn"}),
        enabled: true,
        order_in_pipeline: 1,
    };

    let tracing_plugin = PipelinePluginConfigDto {
        plugin_type: PluginType::Tracing,
        config_data: json!({
            "endpoint": "http://updated-trace.example.com/v1/traces",
            "api_key": {
                "type": "literal",
                "value": "updated-key"
            }
        }),
        enabled: true,
        order_in_pipeline: 2,
    };

    let update_req = UpdatePipelineRequestDto {
        name: None,
        pipeline_type: None,
        description: Some("Updated with logging and tracing".to_string()),
        plugins: Some(vec![logging_plugin, tracing_plugin]),
        enabled: None,
    };

    let update_response = server
        .put(&format!(
            "/api/v1/management/pipelines/{}",
            created_pipeline.id
        ))
        .json(&update_req)
        .await;
    update_response.assert_status_ok();

    let updated_pipeline: PipelineResponseDto = update_response.json();
    assert_eq!(updated_pipeline.plugins.len(), 2);

    // Verify logging plugin
    let logging_plugin = updated_pipeline
        .plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Logging)
        .unwrap();
    let logging_config: LoggingConfigDto =
        serde_json::from_value(logging_plugin.config_data.clone()).unwrap();
    assert_eq!(logging_config.level, "warn");

    // Verify tracing plugin
    let tracing_plugin = updated_pipeline
        .plugins
        .iter()
        .find(|p| p.plugin_type == PluginType::Tracing)
        .unwrap();
    let tracing_config: TracingConfigDto =
        serde_json::from_value(tracing_plugin.config_data.clone()).unwrap();
    assert_eq!(
        tracing_config.endpoint,
        "http://updated-trace.example.com/v1/traces"
    );
    assert_eq!(
        tracing_config.api_key,
        SecretObject::literal("updated-key".to_string())
    );
}

/*
Further considerations for tests:
- Test with different plugin types if more are added.
- Test updating plugin order.
- Test specific error messages or structures in ApiError responses if needed.
- Test pagination if list_pipelines implements it.
*/
