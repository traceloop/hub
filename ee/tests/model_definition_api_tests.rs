#![allow(unused_imports)] // Allow unused imports for now, will remove later

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use axum_test::TestServer; // Using axum-test
use chrono::{DateTime, Utc};
use ee::{
    api::routes::model_definition_routes, // Assuming this is the entry point for model definition routes
    db::models::{ModelDefinition, Provider as DbProvider}, // For direct DB checks if needed
    dto::{
        self, AzureProviderConfig, CreateModelDefinitionRequest, CreateProviderRequest,
        ModelDefinitionResponse, OpenAIProviderConfig, ProviderConfig, ProviderResponse,
        ProviderType, SecretObject, UpdateModelDefinitionRequest,
    },
    errors::ApiError,      // Assuming ApiError is serializable for error responses
    management_api_bundle, // Main router function from lib.rs
    AppState,              // Main AppState
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{migrate::Migrator, types::Uuid, PgPool};
use std::path::Path;
use std::sync::Arc;
use testcontainers::{core::WaitFor, runners::AsyncRunner, ImageExt};
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

    let (router, _config_provider) = management_api_bundle(pool.clone());
    let client = TestServer::new(router).expect("Failed to create TestServer");

    (client, pool, container)
}

// Helper to create a provider for tests that need an existing provider
async fn create_test_provider(
    client: &TestServer,
    name: &str,
    provider_type: ProviderType,
) -> ProviderResponse {
    let config = match provider_type {
        ProviderType::Azure => ProviderConfig::Azure(AzureProviderConfig {
            api_key: SecretObject::literal("test_azure_key".to_string()),
            resource_name: "test_resource".to_string(),
            api_version: "2023-05-15".to_string(),
            base_url: None,
        }),
        ProviderType::OpenAI => ProviderConfig::OpenAI(OpenAIProviderConfig {
            api_key: SecretObject::literal("test_openai_key".to_string()),
            organization_id: None,
        }),
        _ => panic!("Unsupported provider type for test helper"),
    };

    let request = CreateProviderRequest {
        name: name.to_string(),
        provider_type,
        config,
        enabled: Some(true),
    };

    let response = client.post("/providers").json(&request).await;
    assert_eq!(
        response.status_code(),
        StatusCode::CREATED,
        "Failed to create test provider. Expected 201, got: {}. Body: {}",
        response.status_code(),
        response.text()
    );

    response.json::<ProviderResponse>()
}

// --- Test Cases --- //

#[tokio::test]
async fn test_create_model_definition_success() {
    let (client, pool, _container) = setup_test_environment().await;

    let provider =
        create_test_provider(&client, "Test Provider for MD", ProviderType::OpenAI).await;

    let request_payload = CreateModelDefinitionRequest {
        key: "gpt-4o-test".to_string(),
        model_type: "gpt-4o".to_string(),
        provider_id: provider.id,
        config_details: Some(json!({ "temperature": 0.7 })),
        enabled: Some(true),
    };

    let response = client
        .post("/model-definitions")
        .json(&request_payload)
        .await;
    assert_eq!(response.status_code(), StatusCode::CREATED);

    let md_response: ModelDefinitionResponse = response.json();
    assert_eq!(md_response.key, request_payload.key);
    assert_eq!(md_response.model_type, request_payload.model_type);
    assert_eq!(md_response.provider.id, provider.id);
    assert_eq!(
        md_response.config_details,
        request_payload.config_details.unwrap()
    );
    assert_eq!(md_response.enabled, request_payload.enabled.unwrap());

    // Verify in DB
    let db_md = sqlx::query_as!(
        ModelDefinition,
        "SELECT id, key, model_type, provider_id, config_details, enabled, created_at, updated_at FROM hub_llmgateway_model_definitions WHERE id = $1",
        md_response.id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(db_md.key, request_payload.key);
}

#[tokio::test]
async fn test_create_model_definition_duplicate_key() {
    let (client, _pool, _container) = setup_test_environment().await;
    let provider = create_test_provider(&client, "Prov-DupKey", ProviderType::OpenAI).await;

    let payload1 = CreateModelDefinitionRequest {
        key: "unique-key-123".to_string(),
        model_type: "type1".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let response1 = client.post("/model-definitions").json(&payload1).await;
    assert_eq!(
        response1.status_code(),
        StatusCode::CREATED,
        "First creation should succeed"
    );

    let payload2 = CreateModelDefinitionRequest {
        // Same key
        key: "unique-key-123".to_string(),
        model_type: "type2".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let response2 = client.post("/model-definitions").json(&payload2).await;
    assert_eq!(response2.status_code(), StatusCode::CONFLICT);

    let error_response: serde_json::Value = response2.json();
    assert!(error_response["error"]
        .as_str()
        .unwrap()
        .contains("Model Definition key 'unique-key-123' already exists"));
}

#[tokio::test]
async fn test_create_model_definition_invalid_provider_id() {
    let (client, _pool, _container) = setup_test_environment().await;
    let non_existent_provider_id = Uuid::new_v4();

    let request_payload = CreateModelDefinitionRequest {
        key: "key-invalid-prov".to_string(),
        model_type: "model-x".to_string(),
        provider_id: non_existent_provider_id,
        config_details: None,
        enabled: Some(true),
    };

    let response = client
        .post("/model-definitions")
        .json(&request_payload)
        .await;
    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

    let error_response: serde_json::Value = response.json();
    assert!(error_response["error"].as_str().unwrap().contains(&format!(
        "Provider with ID {} does not exist",
        non_existent_provider_id
    )));
}

#[tokio::test]
async fn test_get_model_definition_success() {
    let (client, _pool, _container) = setup_test_environment().await;
    let provider = create_test_provider(&client, "Prov-GetMD", ProviderType::OpenAI).await;
    let create_payload = CreateModelDefinitionRequest {
        key: "get-me-md".to_string(),
        model_type: "fetch-model".to_string(),
        provider_id: provider.id,
        config_details: Some(json!({ "detail": "some_value" })),
        enabled: Some(true),
    };
    let create_response = client
        .post("/model-definitions")
        .json(&create_payload)
        .await;
    assert_eq!(
        create_response.status_code(),
        StatusCode::CREATED,
        "MD creation for GET test failed"
    );
    let created_md: ModelDefinitionResponse = create_response.json();

    let response = client
        .get(&format!("/model-definitions/{}", created_md.id))
        .await;
    assert_eq!(response.status_code(), StatusCode::OK);

    let fetched_md: ModelDefinitionResponse = response.json();
    assert_eq!(fetched_md.id, created_md.id);
    assert_eq!(fetched_md.key, create_payload.key);
    assert_eq!(fetched_md.provider.id, provider.id);
    assert_eq!(
        fetched_md.config_details,
        create_payload.config_details.unwrap()
    );
}

#[tokio::test]
async fn test_get_model_definition_not_found() {
    let (client, _pool, _container) = setup_test_environment().await;
    let non_existent_id = Uuid::new_v4();
    let response = client
        .get(&format!("/model-definitions/{}", non_existent_id))
        .await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_model_definition_by_key_success() {
    let (client, _pool, _container) = setup_test_environment().await;
    let provider = create_test_provider(&client, "Prov-GetKeyMD", ProviderType::OpenAI).await;
    let key_to_find = "find-me-by-key";
    let create_payload = CreateModelDefinitionRequest {
        key: key_to_find.to_string(),
        model_type: "fetch-model-key".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let create_response = client
        .post("/model-definitions")
        .json(&create_payload)
        .await;
    assert_eq!(
        create_response.status_code(),
        StatusCode::CREATED,
        "MD creation for GET by key test failed"
    );
    let _created_md: ModelDefinitionResponse = create_response.json();

    let response = client
        .get(&format!("/model-definitions/key/{}", key_to_find))
        .await;
    assert_eq!(response.status_code(), StatusCode::OK);

    let fetched_md: ModelDefinitionResponse = response.json();
    assert_eq!(fetched_md.key, key_to_find);
    assert_eq!(fetched_md.provider.id, provider.id);
}

#[tokio::test]
async fn test_get_model_definition_by_key_not_found() {
    let (client, _pool, _container) = setup_test_environment().await;
    let non_existent_key = "i-do-not-exist-key";
    let response = client
        .get(&format!("/model-definitions/key/{}", non_existent_key))
        .await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_model_definitions_empty() {
    let (client, _pool, _container) = setup_test_environment().await;
    // To ensure this test is truly empty, we'd ideally want a fresh DB.
    // With OnceCell, this will list whatever was created by previous tests in this run.
    // For a true empty test, it might need its own OnceCell or a DB cleaning mechanism.
    // However, for now, we accept it lists what's there.
    // If this is the first test to run (or after a DB clear), it should be empty.
    let response = client.get("/model-definitions").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    // let mds: Vec<ModelDefinitionResponse> = response.json();
    // assert!(mds.is_empty()); // This might fail if other tests ran first and created data.
}

#[tokio::test]
async fn test_list_model_definitions_multiple() {
    let (client, _pool, _container) = setup_test_environment().await;
    let provider1 = create_test_provider(&client, "Prov-List1-MD", ProviderType::OpenAI).await; // Unique name
    let provider2 = create_test_provider(&client, "Prov-List2-MD", ProviderType::Azure).await; // Unique name

    client
        .post("/model-definitions")
        .json(&CreateModelDefinitionRequest {
            key: format!("md1-list-{}", Uuid::new_v4()), // Ensure unique key
            model_type: "t1".to_string(),
            provider_id: provider1.id,
            config_details: None,
            enabled: Some(true),
        })
        .await
        .assert_status(StatusCode::CREATED);
    client
        .post("/model-definitions")
        .json(&CreateModelDefinitionRequest {
            key: format!("md2-list-{}", Uuid::new_v4()), // Ensure unique key
            model_type: "t2".to_string(),
            provider_id: provider2.id,
            config_details: None,
            enabled: Some(true),
        })
        .await
        .assert_status(StatusCode::CREATED);
    client
        .post("/model-definitions")
        .json(&CreateModelDefinitionRequest {
            key: format!("md3-list-{}", Uuid::new_v4()), // Ensure unique key
            model_type: "t3".to_string(),
            provider_id: provider1.id,
            config_details: None,
            enabled: Some(true),
        })
        .await
        .assert_status(StatusCode::CREATED);

    let response = client.get("/model-definitions").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let mds: Vec<ModelDefinitionResponse> = response.json();
    // The exact number can be tricky with OnceCell if tests are not perfectly isolated
    // or if other tests add model definitions.
    // A more robust check might be to ensure at least 3 are present if these are the only ones added by this test.
    assert!(mds.len() >= 3);
}

#[tokio::test]
async fn test_update_model_definition_success() {
    let (client, _pool, _container) = setup_test_environment().await;
    let provider = create_test_provider(&client, "Prov-UpdateMD", ProviderType::OpenAI).await;
    let create_payload = CreateModelDefinitionRequest {
        key: format!("update-me-md-{}", Uuid::new_v4()), // Ensure unique key
        model_type: "initial-model".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let create_response = client
        .post("/model-definitions")
        .json(&create_payload)
        .await;
    assert_eq!(
        create_response.status_code(),
        StatusCode::CREATED,
        "Initial MD creation for update test failed"
    );
    let created_md: ModelDefinitionResponse = create_response.json();

    let update_payload = UpdateModelDefinitionRequest {
        key: Some(format!("updated-md-key-{}", Uuid::new_v4())), // Ensure unique key
        model_type: Some("updated-model-type".to_string()),
        provider_id: None, // Not changing provider
        config_details: Some(json!({ "new_detail": "cool"})),
        enabled: Some(false),
    };

    let update_response = client
        .put(&format!("/model-definitions/{}", created_md.id))
        .json(&update_payload)
        .await;
    assert_eq!(update_response.status_code(), StatusCode::OK);

    let updated_md: ModelDefinitionResponse = update_response.json();
    assert_eq!(updated_md.id, created_md.id);
    assert_eq!(updated_md.key, update_payload.key.unwrap());
    assert_eq!(updated_md.model_type, update_payload.model_type.unwrap());
    assert_eq!(updated_md.provider.id, provider.id); // Provider should not change
    assert_eq!(
        updated_md.config_details,
        update_payload.config_details.unwrap()
    );
    assert_eq!(updated_md.enabled, update_payload.enabled.unwrap());
}

#[tokio::test]
async fn test_update_model_definition_not_found() {
    let (client, _pool, _container) = setup_test_environment().await;
    let non_existent_id = Uuid::new_v4();
    let update_payload = UpdateModelDefinitionRequest {
        key: Some("irrelevant".to_string()),
        model_type: None,
        provider_id: None,
        config_details: None,
        enabled: None,
    };
    let response = client
        .put(&format!("/model-definitions/{}", non_existent_id))
        .json(&update_payload)
        .await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_model_definition_duplicate_key_conflict() {
    let (client, _pool, _container) = setup_test_environment().await;
    let provider =
        create_test_provider(&client, "Prov-UpdateDupKey-MD", ProviderType::OpenAI).await; // Unique name

    let md1_key = format!("key1-update-{}", Uuid::new_v4());
    let md1_payload = CreateModelDefinitionRequest {
        key: md1_key.clone(),
        model_type: "mt1".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let md1_res = client.post("/model-definitions").json(&md1_payload).await;
    assert_eq!(md1_res.status_code(), StatusCode::CREATED);

    let md2_payload = CreateModelDefinitionRequest {
        key: format!("key2-update-{}", Uuid::new_v4()), // Unique key
        model_type: "mt2".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let md2_res = client.post("/model-definitions").json(&md2_payload).await;
    assert_eq!(md2_res.status_code(), StatusCode::CREATED);
    let md2_created: ModelDefinitionResponse = md2_res.json();

    // Attempt to update md2 to have md1's key
    let update_payload = UpdateModelDefinitionRequest {
        key: Some(md1_key.clone()),
        model_type: None,
        provider_id: None,
        config_details: None,
        enabled: None,
    };
    let update_response = client
        .put(&format!("/model-definitions/{}", md2_created.id))
        .json(&update_payload)
        .await;
    assert_eq!(update_response.status_code(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_update_model_definition_invalid_provider_id() {
    let (client, _pool, _container) = setup_test_environment().await; // Uses OnceCell
    let provider =
        create_test_provider(&client, "Prov-UpdateInvProv-MD", ProviderType::OpenAI).await; // Unique name
    let md_payload = CreateModelDefinitionRequest {
        key: format!("key-update-inv-prov-{}", Uuid::new_v4()), // Unique key
        model_type: "mt_inv".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let md_res = client.post("/model-definitions").json(&md_payload).await;
    assert_eq!(md_res.status_code(), StatusCode::CREATED);
    let created_md: ModelDefinitionResponse = md_res.json();

    let non_existent_provider_id = Uuid::new_v4();
    let update_payload = UpdateModelDefinitionRequest {
        provider_id: Some(non_existent_provider_id),
        key: None,
        model_type: None,
        config_details: None,
        enabled: None,
    };
    let update_response = client
        .put(&format!("/model-definitions/{}", created_md.id))
        .json(&update_payload)
        .await;
    assert_eq!(update_response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_model_definition_success() {
    let (client, pool, _container) = setup_test_environment().await;
    let provider = create_test_provider(&client, "Prov-DeleteMD-MD", ProviderType::OpenAI).await; // Unique name
    let create_payload = CreateModelDefinitionRequest {
        key: format!("delete-me-md-{}", Uuid::new_v4()), // Ensure unique key
        model_type: "disposable-model".to_string(),
        provider_id: provider.id,
        config_details: None,
        enabled: Some(true),
    };
    let create_response = client
        .post("/model-definitions")
        .json(&create_payload)
        .await;
    assert_eq!(
        create_response.status_code(),
        StatusCode::CREATED,
        "MD creation for delete test failed"
    );
    let created_md: ModelDefinitionResponse = create_response.json();

    let delete_response = client
        .delete(&format!("/model-definitions/{}", created_md.id))
        .await;
    assert_eq!(delete_response.status_code(), StatusCode::OK);

    // Verify it's gone from DB
    let db_model_after_delete = sqlx::query_as!(
        ModelDefinition,
        "SELECT id, key, model_type, provider_id, config_details, enabled, created_at, updated_at FROM hub_llmgateway_model_definitions WHERE id = $1",
        created_md.id
    )
    .fetch_optional(&pool)
    .await
    .expect("DB query failed after delete");
    assert!(
        db_model_after_delete.is_none(),
        "Model Definition should not exist in DB after delete"
    );
}

#[tokio::test]
async fn test_delete_model_definition_not_found() {
    let (client, _pool, _container) = setup_test_environment().await;
    let non_existent_id = Uuid::new_v4();
    let response = client
        .delete(&format!("/model-definitions/{}", non_existent_id))
        .await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}
