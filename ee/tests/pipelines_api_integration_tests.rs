#![allow(dead_code)] // Allow dead code for work-in-progress
#![allow(unused_imports)] // Allow unused imports for work-in-progress

use axum::http::StatusCode; // For status assertions
use axum::Router as AxumRouter; // Alias to avoid conflict if Router from testcontainers is brought in
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, Executor, PgPool};
use std::path::Path; // For migration path
use std::sync::Arc;

// Crate-specific imports
use ee::{
    // DTOs needed for requests/responses
    dto::{
        AnthropicProviderConfig, AzureProviderConfig, BedrockProviderConfig,
        CreateModelDefinitionRequest, CreatePipelineRequestDto, CreateProviderRequest,
        ModelDefinitionResponse, ModelRouterConfigDto, ModelRouterModelEntryDto,
        ModelRouterStrategyDto, OpenAIProviderConfig, PipelinePluginConfigDto, PipelineResponseDto,
        PluginType, ProviderConfig, ProviderResponse, ProviderType, SecretObject,
        UpdatePipelineRequestDto, VertexAIProviderConfig,
    },
    // Potentially services or repos if we need to setup data directly (though API is preferred)
    // services::{provider_service::ProviderService, model_definition_service::ModelDefinitionService, pipeline_service::PipelineService},
    // db::repositories::{provider_repository::ProviderRepository, model_definition_repository::ModelDefinitionRepository, pipeline_repository::PipelineRepository},
    ee_api_bundle,
    AppState,
};

// Test framework imports
use axum_test::TestServer; // For testing the Axum app
use serde_json::json;
use testcontainers::{core::WaitFor, runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

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
        .max_connections(20)
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

    let (router, _config_provider) = ee_api_bundle(pool.clone());
    let client = TestServer::new(router).expect("Failed to create TestServer");

    (client, pool, container) // Return Arc<PgPool> and container
}

// --- Test Cases Start Here ---

#[tokio::test]
async fn test_health_check() {
    let (server, _pool, _container) = setup_test_environment().await;
    let response = server.get("/health").await;
    response.assert_status_ok();
    response.assert_text("EE API is healthy");
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

    let response = server.post("/providers").json(&request_payload).await;
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
    let response = server.post("/model-definitions").json(&request).await;
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
    let response = server.post("/pipelines").json(&pipeline_req).await;
    response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = response.json();
    assert_eq!(created_pipeline.name, pipeline_req.name);
    assert!(created_pipeline.plugins.is_empty());
    let fetched_response = server
        .get(&format!("/pipelines/{}", created_pipeline.id))
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
    let creation_response = server.post("/pipelines").json(&pipeline_req).await;
    creation_response.assert_status(StatusCode::CREATED);

    let conflict_response = server.post("/pipelines").json(&pipeline_req).await;
    conflict_response.assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_get_pipeline_not_found() {
    let (server, _pool, _container) = setup_test_environment().await;
    let non_existent_id = Uuid::new_v4();
    server
        .get(&format!("/pipelines/{}", non_existent_id))
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
    let response = server.post("/pipelines").json(&pipeline_req).await;
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
    let response = server.post("/pipelines").json(&pipeline_req).await;
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
    let response1 = server.post("/pipelines").json(&pipeline1_req).await;
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
    let response2 = server.post("/pipelines").json(&pipeline2_req).await;
    response2.assert_status(StatusCode::CREATED);
    let created_pipeline2: PipelineResponseDto = response2.json();

    let list_response = server.get("/pipelines").await;
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
    let creation_response = server.post("/pipelines").json(&pipeline_req).await;
    creation_response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = creation_response.json();

    let fetch_response = server
        .get(&format!("/pipelines/name/{}", pipeline_name))
        .await;
    fetch_response.assert_status_ok();
    let fetched_pipeline: PipelineResponseDto = fetch_response.json();
    assert_eq!(fetched_pipeline.id, created_pipeline.id);
    assert_eq!(fetched_pipeline.name, pipeline_name);
    let non_existent_name = format!("non-existent-name-{}", Uuid::new_v4());
    server
        .get(&format!("/pipelines/name/{}", non_existent_name))
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
    let creation_response = server.post("/pipelines").json(&create_req).await;
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
        config_data: json!({ "filter_level": "strict"}),
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
        .put(&format!("/pipelines/{}", created_pipeline.id))
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
        plugin2
            .config_data
            .get("filter_level")
            .unwrap()
            .as_str()
            .unwrap(),
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
    let creation_response = server.post("/pipelines").json(&pipeline_req).await;
    creation_response.assert_status(StatusCode::CREATED);
    let created_pipeline: PipelineResponseDto = creation_response.json();

    let delete_response = server
        .delete(&format!("/pipelines/{}", created_pipeline.id))
        .await;
    delete_response.assert_status_ok();
    server
        .get(&format!("/pipelines/{}", created_pipeline.id))
        .await
        .assert_status_not_found();
    let non_existent_id = Uuid::new_v4();
    server
        .delete(&format!("/pipelines/{}", non_existent_id))
        .await
        .assert_status_not_found();
}

/*
Further considerations for tests:
- Test with different plugin types if more are added.
- Test updating plugin order.
- Test specific error messages or structures in ApiError responses if needed.
- Test pagination if list_pipelines implements it.
*/
