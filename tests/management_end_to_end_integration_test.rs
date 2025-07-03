use axum_test::TestServer;
use hub_lib::{routes, state::AppState};
use std::sync::Arc;

// Always import database components since they're now always available
use ee::db_based_config_integration;
use sqlx::PgPool;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;

mod management_integration_tests {
    use super::*;
    use serde_json::json;

    struct TestContext {
        _container: ContainerAsync<Postgres>,
        _pool: PgPool,
        test_server: TestServer,
    }

    async fn setup_test_context() -> TestContext {
        let postgres_image = Postgres::default()
            .with_db_name("test_db")
            .with_user("test_user")
            .with_password("test_password");

        let container = postgres_image.start().await.expect("Failed to start container");
        let host_port = container.get_host_port_ipv4(5432).await.expect("Failed to get port");

        let database_url = format!(
            "postgresql://test_user:test_password@localhost:{}/test_db",
            host_port
        );

        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        sqlx::migrate!("ee/migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        let db_integration = db_based_config_integration(pool.clone())
            .await
            .expect("Failed to initialize database integration");

        // Create a simple test router with just the management API to avoid Prometheus conflicts
        let test_router = axum::Router::new()
            .route("/health", axum::routing::get(|| async { "OK" }))
            .nest("/api/v1/management", db_integration.router.clone());

        let test_server = TestServer::new(test_router).expect("Failed to create test server");

        TestContext {
            _container: container,
            _pool: pool,
            test_server,
        }
    }

    #[tokio::test]
    async fn test_provider_crud_operations() {
        let ctx = setup_test_context().await;

        // Test creating a provider
        let create_provider_request = json!({
            "name": "test_openai",
            "provider_type": "openai",
            "config": {
                "api_key": {
                    "type": "literal",
                    "value": "sk-test123",
                    "encrypted": false
                }
            }
        });

        let response = ctx
            .test_server
            .post("/api/v1/management/providers")
            .json(&create_provider_request)
            .await;

        assert_eq!(response.status_code(), 201);

        let provider_response: serde_json::Value = response.json();
        let provider_id = provider_response["id"].as_str().unwrap();

        // Test listing providers
        let list_response = ctx
            .test_server
            .get("/api/v1/management/providers")
            .await;

        assert_eq!(list_response.status_code(), 200);

        let providers: serde_json::Value = list_response.json();
        assert!(providers.as_array().unwrap().len() >= 1);

        // Test getting specific provider
        let get_response = ctx
            .test_server
            .get(&format!("/api/v1/management/providers/{}", provider_id))
            .await;

        assert_eq!(get_response.status_code(), 200);

        let provider: serde_json::Value = get_response.json();
        assert_eq!(provider["name"], "test_openai");

        // Test updating provider
        let update_request = json!({
            "name": "updated_openai",
            "provider_type": "openai",
            "config": {
                "api_key": {
                    "type": "literal", 
                    "value": "sk-updated123",
                    "encrypted": false
                }
            }
        });

        let update_response = ctx
            .test_server
            .put(&format!("/api/v1/management/providers/{}", provider_id))
            .json(&update_request)
            .await;

        assert_eq!(update_response.status_code(), 200);

        // Test deleting provider
        let delete_response = ctx
            .test_server
            .delete(&format!("/api/v1/management/providers/{}", provider_id))
            .await;

        assert_eq!(delete_response.status_code(), 200);

        // Verify provider is deleted
        let get_deleted_response = ctx
            .test_server
            .get(&format!("/api/v1/management/providers/{}", provider_id))
            .await;

        assert_eq!(get_deleted_response.status_code(), 404);
    }

    #[tokio::test]
    async fn test_model_definition_crud_operations() {
        let ctx = setup_test_context().await;

        // First create a provider
        let create_provider_request = json!({
            "name": "test_provider",
            "provider_type": "openai",
            "config": {
                "api_key": {
                    "type": "literal",
                    "value": "sk-test123",
                    "encrypted": false
                }
            }
        });

        let provider_response = ctx
            .test_server
            .post("/api/v1/management/providers")
            .json(&create_provider_request)
            .await;

        assert_eq!(provider_response.status_code(), 201);

        let provider_data: serde_json::Value = provider_response.json();
        let provider_id = provider_data["id"].as_str().unwrap();

        // Create model definition
        let create_model_request = json!({
            "key": "gpt-4-test",
            "model_type": "gpt-4",
            "provider_id": provider_id
        });

        let model_response = ctx
            .test_server
            .post("/api/v1/management/model-definitions")
            .json(&create_model_request)
            .await;

        if model_response.status_code() != 201 {
            println!("Model creation failed with status: {}", model_response.status_code());
            println!("Response body: {}", model_response.text());
            panic!("Expected 201, got {}", model_response.status_code());
        }

        assert_eq!(model_response.status_code(), 201);

        let model_data: serde_json::Value = model_response.json();
        let model_id = model_data["id"].as_str().unwrap();

        // Test listing models
        let list_response = ctx
            .test_server
            .get("/api/v1/management/model-definitions")
            .await;

        assert_eq!(list_response.status_code(), 200);

        let models: serde_json::Value = list_response.json();
        assert!(models.as_array().unwrap().len() >= 1);

        // Test getting specific model
        let get_response = ctx
            .test_server
            .get(&format!("/api/v1/management/model-definitions/{}", model_id))
            .await;

        assert_eq!(get_response.status_code(), 200);

        let model: serde_json::Value = get_response.json();
        assert_eq!(model["key"], "gpt-4-test");

        // Test updating model
        let update_request = json!({
            "key": "gpt-4-updated",
            "model_type": "gpt-4",
            "provider_id": provider_id
        });

        let update_response = ctx
            .test_server
            .put(&format!("/api/v1/management/model-definitions/{}", model_id))
            .json(&update_request)
            .await;

        assert_eq!(update_response.status_code(), 200);

        // Test deleting model
        let delete_response = ctx
            .test_server
            .delete(&format!("/api/v1/management/model-definitions/{}", model_id))
            .await;

        assert_eq!(delete_response.status_code(), 200);

        // Clean up provider
        ctx.test_server
            .delete(&format!("/api/v1/management/providers/{}", provider_id))
            .await;
    }

    #[tokio::test]
    async fn test_pipeline_crud_operations() {
        let ctx = setup_test_context().await;

        // First create a provider
        let create_provider_request = json!({
            "name": "test_provider",
            "provider_type": "openai", 
            "config": {
                "api_key": {
                    "type": "literal",
                    "value": "sk-test123",
                    "encrypted": false
                }
            }
        });

        let provider_response = ctx
            .test_server
            .post("/api/v1/management/providers")
            .json(&create_provider_request)
            .await;

        let provider_data: serde_json::Value = provider_response.json();
        let provider_id = provider_data["id"].as_str().unwrap();

        // Create model definition
        let create_model_request = json!({
            "key": "gpt-4-test",
            "model_type": "gpt-4",
            "provider_id": provider_id
        });

        let model_response = ctx
            .test_server
            .post("/api/v1/management/model-definitions")
            .json(&create_model_request)
            .await;

        let model_data: serde_json::Value = model_response.json();
        let model_id = model_data["id"].as_str().unwrap();
        let model_key = model_data["key"].as_str().unwrap();

        // Create pipeline
        let create_pipeline_request = json!({
            "name": "test_pipeline",
            "pipeline_type": "chat",
            "plugins": [
                {
                    "plugin_type": "model-router",
                    "config_data": {
                        "models": [{"key": model_key, "priority": 0}],
                        "strategy": "ordered_fallback"
                    }
                }
            ]
        });

        let pipeline_response = ctx
            .test_server
            .post("/api/v1/management/pipelines")
            .json(&create_pipeline_request)
            .await;

        if pipeline_response.status_code() != 201 {
            println!("Pipeline creation failed with status: {}", pipeline_response.status_code());
            println!("Response body: {}", pipeline_response.text());
            panic!("Expected 201, got {}", pipeline_response.status_code());
        }

        assert_eq!(pipeline_response.status_code(), 201);

        let pipeline_data: serde_json::Value = pipeline_response.json();
        let pipeline_id = pipeline_data["id"].as_str().unwrap();

        // Test listing pipelines
        let list_response = ctx
            .test_server
            .get("/api/v1/management/pipelines")
            .await;

        assert_eq!(list_response.status_code(), 200);

        let pipelines: serde_json::Value = list_response.json();
        assert!(pipelines.as_array().unwrap().len() >= 1);

        // Test getting specific pipeline
        let get_response = ctx
            .test_server
            .get(&format!("/api/v1/management/pipelines/{}", pipeline_id))
            .await;

        assert_eq!(get_response.status_code(), 200);

        let pipeline: serde_json::Value = get_response.json();
        assert_eq!(pipeline["name"], "test_pipeline");

        // Test updating pipeline
        let update_request = json!({
            "name": "updated_pipeline",
            "pipeline_type": "chat",
            "plugins": [
                {
                    "plugin_type": "model-router",
                    "config_data": {
                        "models": [{"key": model_key, "priority": 0}],
                        "strategy": "ordered_fallback"
                    }
                }
            ]
        });

        let update_response = ctx
            .test_server
            .put(&format!("/api/v1/management/pipelines/{}", pipeline_id))
            .json(&update_request)
            .await;

        assert_eq!(update_response.status_code(), 200);

        // Test deleting pipeline
        let delete_response = ctx
            .test_server
            .delete(&format!("/api/v1/management/pipelines/{}", pipeline_id))
            .await;

        assert_eq!(delete_response.status_code(), 200);

        // Clean up
        ctx.test_server
            .delete(&format!("/api/v1/management/model-definitions/{}", model_id))
            .await;
        ctx.test_server
            .delete(&format!("/api/v1/management/providers/{}", provider_id))
            .await;
    }

    #[tokio::test]
    async fn test_management_api_error_handling() {
        let ctx = setup_test_context().await;

        // Test creating provider with invalid data
        let invalid_request = json!({
            "name": "",
            "provider_type": "invalid_type",
            "config": {}
        });

        let response = ctx
            .test_server
            .post("/api/v1/management/providers")
            .json(&invalid_request)
            .await;

        assert!(response.status_code().as_u16() >= 400);

        // Test getting non-existent provider
        let get_response = ctx
            .test_server
            .get("/api/v1/management/providers/00000000-0000-0000-0000-000000000000")
            .await;

        assert_eq!(get_response.status_code(), 404);

        // Test deleting non-existent provider
        let delete_response = ctx
            .test_server
            .delete("/api/v1/management/providers/00000000-0000-0000-0000-000000000000")
            .await;

        assert_eq!(delete_response.status_code(), 404);
    }
}

// Test for when database mode is not available (YAML mode)
#[tokio::test]
async fn test_yaml_mode_no_management_api() {
    // Create a simple YAML config
    let yaml_config = hub_gateway_core_types::GatewayConfig {
        providers: vec![],
        models: vec![],
        pipelines: vec![],
        general: None,
    };

    let app_state = Arc::new(
        AppState::new(yaml_config).expect("Failed to create app state"),
    );

    // Create router without management API (YAML mode)
    let main_router = routes::create_router(app_state.clone());

    let test_server = TestServer::new(main_router).expect("Failed to create test server");

    // Test that management endpoints are not available
    let response = test_server
        .get("/api/v1/management/providers")
        .await;

    assert_eq!(response.status_code(), 404);
}
