#![allow(dead_code)] // Allow dead code for now as we build up tests
#![allow(unused_imports)] // Allow unused imports for now

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use axum_test::TestServer;
use chrono::{DateTime, Utc};
use hub_lib::management::{
    AppState,
    api::routes::provider_routes,
    db::models::Provider,
    dto::{
        AnthropicProviderConfig, AzureProviderConfig, BedrockProviderConfig, CreateProviderRequest,
        OpenAIProviderConfig, ProviderConfig, ProviderResponse, ProviderType, SecretObject,
        UpdateProviderRequest, VertexAIProviderConfig,
    },
    errors::ApiError,
    management_api_bundle,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, migrate::Migrator, types::Uuid};
use std::path::Path;
use std::sync::Arc;
use testcontainers::{ImageExt, core::WaitFor, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

// Test data structure for parametrized tests
#[derive(Clone, Debug)]
struct ProviderTestData {
    name: String,
    provider_type: ProviderType,
    config: ProviderConfig,
    updated_config: ProviderConfig,
}

// Generate test data for all provider types
fn get_all_provider_test_data() -> Vec<ProviderTestData> {
    vec![
        ProviderTestData {
            name: "Test OpenAI Provider".to_string(),
            provider_type: ProviderType::OpenAI,
            config: ProviderConfig::OpenAI(OpenAIProviderConfig {
                api_key: SecretObject::literal("test_openai_key".to_string()),
                organization_id: Some("test_org".to_string()),
            }),
            updated_config: ProviderConfig::OpenAI(OpenAIProviderConfig {
                api_key: SecretObject::literal("updated_openai_key".to_string()),
                organization_id: Some("updated_org".to_string()),
            }),
        },
        ProviderTestData {
            name: "Test Azure Provider".to_string(),
            provider_type: ProviderType::Azure,
            config: ProviderConfig::Azure(AzureProviderConfig {
                api_key: SecretObject::literal("test_azure_key".to_string()),
                resource_name: "test_resource".to_string(),
                api_version: "2023-05-15".to_string(),
                base_url: None,
            }),
            updated_config: ProviderConfig::Azure(AzureProviderConfig {
                api_key: SecretObject::literal("updated_azure_key".to_string()),
                resource_name: "updated_resource".to_string(),
                api_version: "2024-02-01".to_string(),
                base_url: None,
            }),
        },
        ProviderTestData {
            name: "Test Anthropic Provider".to_string(),
            provider_type: ProviderType::Anthropic,
            config: ProviderConfig::Anthropic(AnthropicProviderConfig {
                api_key: SecretObject::literal("test_anthropic_key".to_string()),
            }),
            updated_config: ProviderConfig::Anthropic(AnthropicProviderConfig {
                api_key: SecretObject::literal("updated_anthropic_key".to_string()),
            }),
        },
        ProviderTestData {
            name: "Test Bedrock Provider".to_string(),
            provider_type: ProviderType::Bedrock,
            config: ProviderConfig::Bedrock(BedrockProviderConfig {
                aws_access_key_id: Some(SecretObject::literal("test_access_key".to_string())),
                aws_secret_access_key: Some(SecretObject::literal("test_secret_key".to_string())),
                aws_session_token: None,
                region: "us-east-1".to_string(),
                use_iam_role: Some(false),
                inference_profile_id: None,
            }),
            updated_config: ProviderConfig::Bedrock(BedrockProviderConfig {
                aws_access_key_id: Some(SecretObject::literal("updated_access_key".to_string())),
                aws_secret_access_key: Some(SecretObject::literal(
                    "updated_secret_key".to_string(),
                )),
                aws_session_token: Some(SecretObject::literal("session_token".to_string())),
                region: "us-west-2".to_string(),
                use_iam_role: Some(false),
                inference_profile_id: None,
            }),
        },
        ProviderTestData {
            name: "Test VertexAI Provider".to_string(),
            provider_type: ProviderType::VertexAI,
            config: ProviderConfig::VertexAI(VertexAIProviderConfig {
                project_id: Some("test-project-123".to_string()),
                location: Some("us-central1".to_string()),
                credentials_path: Some("/path/to/service-account.json".to_string()),
                api_key: None,
            }),
            updated_config: ProviderConfig::VertexAI(VertexAIProviderConfig {
                project_id: Some("updated-project-456".to_string()),
                location: Some("europe-west1".to_string()),
                credentials_path: None,
                api_key: Some(SecretObject::literal("updated_vertex_api_key".to_string())),
            }),
        },
    ]
}

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

#[tokio::test]
async fn test_create_provider_success() {
    let (client, _pool, _container) = setup_test_environment().await;

    let request_payload = CreateProviderRequest {
        name: "Test OpenAI Provider".to_string(),
        provider_type: ProviderType::OpenAI,
        config: ProviderConfig::OpenAI(OpenAIProviderConfig {
            api_key: SecretObject::literal("test_openai_key".to_string()),
            organization_id: Some("test_org".to_string()),
        }),
        enabled: Some(true),
    };

    let before_request = Utc::now();
    let response = client
        .post("/api/v1/management/providers")
        .json(&request_payload)
        .await;
    let after_request = Utc::now();

    assert_eq!(response.status_code(), axum::http::StatusCode::CREATED);

    let provider_response: ProviderResponse = response.json::<ProviderResponse>();

    assert_eq!(provider_response.name, request_payload.name);
    assert_eq!(
        provider_response.provider_type,
        request_payload.provider_type
    );
    assert_eq!(provider_response.config, request_payload.config);
    assert_eq!(provider_response.enabled, request_payload.enabled.unwrap());
    assert!(provider_response.id != Uuid::nil());

    // Robust timestamp assertions with time buffer for clock skew
    let time_buffer = chrono::Duration::seconds(2);
    assert!(provider_response.created_at >= before_request - time_buffer);
    assert!(provider_response.created_at <= after_request + time_buffer);
    assert!(provider_response.updated_at >= before_request - time_buffer);
    assert!(provider_response.updated_at <= after_request + time_buffer);
}

#[tokio::test]
async fn test_create_vertexai_provider_success() {
    let (client, _pool, _container) = setup_test_environment().await;

    let request_payload = CreateProviderRequest {
        name: "Test VertexAI Provider".to_string(),
        provider_type: ProviderType::VertexAI,
        config: ProviderConfig::VertexAI(VertexAIProviderConfig {
            project_id: Some("test-project-123".to_string()),
            location: Some("us-central1".to_string()),
            credentials_path: Some("/path/to/service-account.json".to_string()),
            api_key: None,
        }),
        enabled: Some(true),
    };

    let before_request = Utc::now();
    let response = client
        .post("/api/v1/management/providers")
        .json(&request_payload)
        .await;
    let after_request = Utc::now();

    assert_eq!(response.status_code(), axum::http::StatusCode::CREATED);

    let provider_response: ProviderResponse = response.json::<ProviderResponse>();

    assert_eq!(provider_response.name, request_payload.name);
    assert_eq!(
        provider_response.provider_type,
        request_payload.provider_type
    );
    assert_eq!(provider_response.config, request_payload.config);
    assert_eq!(provider_response.enabled, request_payload.enabled.unwrap());
    assert!(provider_response.id != Uuid::nil());

    // Robust timestamp assertions with time buffer for clock skew
    let time_buffer = chrono::Duration::seconds(2);
    assert!(provider_response.created_at >= before_request - time_buffer);
    assert!(provider_response.created_at <= after_request + time_buffer);
    assert!(provider_response.updated_at >= before_request - time_buffer);
    assert!(provider_response.updated_at <= after_request + time_buffer);
}

#[tokio::test]
async fn test_create_vertexai_provider_with_api_key() {
    let (client, _pool, _container) = setup_test_environment().await;

    let request_payload = CreateProviderRequest {
        name: "Test VertexAI Provider with API Key".to_string(),
        provider_type: ProviderType::VertexAI,
        config: ProviderConfig::VertexAI(VertexAIProviderConfig {
            project_id: Some("test-project-456".to_string()),
            location: Some("europe-west1".to_string()),
            credentials_path: None,
            api_key: Some(SecretObject::literal("test-vertex-api-key".to_string())),
        }),
        enabled: Some(false),
    };

    let response = client
        .post("/api/v1/management/providers")
        .json(&request_payload)
        .await;

    assert_eq!(response.status_code(), axum::http::StatusCode::CREATED);

    let provider_response: ProviderResponse = response.json::<ProviderResponse>();

    assert_eq!(provider_response.name, request_payload.name);
    assert_eq!(
        provider_response.provider_type,
        request_payload.provider_type
    );
    assert_eq!(provider_response.config, request_payload.config);
    assert_eq!(provider_response.enabled, request_payload.enabled.unwrap());
}

#[tokio::test]
async fn test_create_provider_duplicate_name() {
    let (client, _pool, _container) = setup_test_environment().await;

    let initial_payload = CreateProviderRequest {
        name: "Unique Name Provider".to_string(),
        provider_type: ProviderType::OpenAI,
        config: ProviderConfig::OpenAI(OpenAIProviderConfig {
            api_key: SecretObject::literal("openai_key_1".to_string()),
            organization_id: None,
        }),
        enabled: Some(true),
    };

    let response1 = client
        .post("/api/v1/management/providers")
        .json(&initial_payload)
        .await;
    assert_eq!(
        response1.status_code(),
        axum::http::StatusCode::CREATED,
        "First provider creation failed"
    );

    let duplicate_payload = CreateProviderRequest {
        name: "Unique Name Provider".to_string(),
        provider_type: ProviderType::Azure,
        config: ProviderConfig::Azure(AzureProviderConfig {
            api_key: SecretObject::literal("azure_key_2".to_string()),
            resource_name: "res2".to_string(),
            api_version: "v2".to_string(),
            base_url: None,
        }),
        enabled: Some(false),
    };

    let response2 = client
        .post("/api/v1/management/providers")
        .json(&duplicate_payload)
        .await;

    assert_eq!(response2.status_code(), axum::http::StatusCode::CONFLICT);

    let error_response: serde_json::Value = response2.json::<serde_json::Value>();
    assert_eq!(
        error_response["error"],
        "Provider with name 'Unique Name Provider' already exists."
    );
}

#[tokio::test]
async fn test_get_provider_success() {
    let (client, _pool, _container) = setup_test_environment().await;

    let request_payload = CreateProviderRequest {
        name: "Provider to GET".to_string(),
        provider_type: ProviderType::Bedrock,
        config: ProviderConfig::Bedrock(BedrockProviderConfig {
            aws_access_key_id: Some(SecretObject::literal("bedrock_access_key".to_string())),
            aws_secret_access_key: Some(SecretObject::literal("bedrock_secret_key".to_string())),
            aws_session_token: None,
            region: "us-east-1".to_string(),
            use_iam_role: Some(false),
            inference_profile_id: None,
        }),
        enabled: Some(true),
    };

    let create_response = client
        .post("/api/v1/management/providers")
        .json(&request_payload)
        .await;
    assert_eq!(
        create_response.status_code(),
        axum::http::StatusCode::CREATED,
        "Provider creation for GET test failed"
    );
    let created_provider: ProviderResponse = create_response.json::<ProviderResponse>();

    let get_response = client
        .get(&format!(
            "/api/v1/management/providers/{}",
            created_provider.id
        ))
        .await;

    assert_eq!(get_response.status_code(), axum::http::StatusCode::OK);

    let fetched_provider: ProviderResponse = get_response.json::<ProviderResponse>();

    assert_eq!(fetched_provider.id, created_provider.id);
    assert_eq!(fetched_provider.name, request_payload.name);
    assert_eq!(
        fetched_provider.provider_type,
        request_payload.provider_type
    );
    assert_eq!(fetched_provider.config, request_payload.config);
    assert_eq!(fetched_provider.enabled, request_payload.enabled.unwrap());
    assert_eq!(fetched_provider.created_at, created_provider.created_at);
    assert_eq!(fetched_provider.updated_at, created_provider.updated_at);
}

#[tokio::test]
async fn test_get_provider_not_found() {
    let (client, _pool, _container) = setup_test_environment().await;

    let non_existent_uuid = Uuid::new_v4();

    let response = client
        .get(&format!(
            "/api/v1/management/providers/{}",
            non_existent_uuid
        ))
        .await;

    assert_eq!(response.status_code(), axum::http::StatusCode::NOT_FOUND);

    let error_response: serde_json::Value = response.json::<serde_json::Value>();
    assert!(error_response["error"].as_str().unwrap().contains(&format!(
        "Provider with ID {} not found.",
        non_existent_uuid
    )));
}

#[tokio::test]
async fn test_list_providers_empty() {
    let (client, _pool, _container) = setup_test_environment().await;

    let response = client.get("/api/v1/management/providers").await;

    assert_eq!(response.status_code(), axum::http::StatusCode::OK);

    let providers: Vec<ProviderResponse> = response.json::<Vec<ProviderResponse>>();
    assert!(providers.is_empty());
}

#[tokio::test]
async fn test_list_providers_multiple() {
    let (client, _pool, _container) = setup_test_environment().await;

    let provider1_payload = CreateProviderRequest {
        name: "List Provider B - OpenAI".to_string(),
        provider_type: ProviderType::OpenAI,
        config: ProviderConfig::OpenAI(OpenAIProviderConfig {
            api_key: SecretObject::literal("key1".to_string()),
            organization_id: None,
        }),
        enabled: Some(true),
    };
    let res1 = client
        .post("/api/v1/management/providers")
        .json(&provider1_payload)
        .await;
    assert_eq!(
        res1.status_code(),
        axum::http::StatusCode::CREATED,
        "Failed to create provider 1 for list test"
    );
    let provider1_resp: ProviderResponse = res1.json::<ProviderResponse>();

    let provider2_payload = CreateProviderRequest {
        name: "List Provider A - Azure".to_string(),
        provider_type: ProviderType::Azure,
        config: ProviderConfig::Azure(AzureProviderConfig {
            api_key: SecretObject::literal("key2".to_string()),
            resource_name: "res2".to_string(),
            api_version: "v2".to_string(),
            base_url: None,
        }),
        enabled: Some(false),
    };
    let res2 = client
        .post("/api/v1/management/providers")
        .json(&provider2_payload)
        .await;
    assert_eq!(
        res2.status_code(),
        axum::http::StatusCode::CREATED,
        "Failed to create provider 2 for list test"
    );
    let provider2_resp: ProviderResponse = res2.json::<ProviderResponse>();

    let list_response = client.get("/api/v1/management/providers").await;
    assert_eq!(list_response.status_code(), axum::http::StatusCode::OK);

    let listed_providers: Vec<ProviderResponse> = list_response.json::<Vec<ProviderResponse>>();

    assert_eq!(listed_providers.len(), 2);

    assert_eq!(listed_providers[0].name, provider2_payload.name);
    assert_eq!(listed_providers[0].id, provider2_resp.id);
    assert_eq!(listed_providers[1].name, provider1_payload.name);
    assert_eq!(listed_providers[1].id, provider1_resp.id);

    assert_eq!(
        listed_providers[0].provider_type,
        provider2_payload.provider_type
    );
    assert_eq!(
        listed_providers[1].provider_type,
        provider1_payload.provider_type
    );
}

#[tokio::test]
async fn test_update_provider_success() {
    let (client, pool, _container) = setup_test_environment().await;

    let initial_payload = CreateProviderRequest {
        name: "Initial Provider Name".to_string(),
        provider_type: ProviderType::OpenAI,
        config: ProviderConfig::OpenAI(OpenAIProviderConfig {
            api_key: SecretObject::literal("initial_openai_key".to_string()),
            organization_id: Some("org_initial".to_string()),
        }),
        enabled: Some(true),
    };
    let create_response = client
        .post("/api/v1/management/providers")
        .json(&initial_payload)
        .await;
    assert_eq!(
        create_response.status_code(),
        axum::http::StatusCode::CREATED,
        "Initial provider creation for update test failed"
    );
    let created_provider: ProviderResponse = create_response.json::<ProviderResponse>();

    let updated_name = "Updated Provider Name".to_string();
    let updated_config = ProviderConfig::OpenAI(OpenAIProviderConfig {
        api_key: SecretObject::literal("updated_openai_key".to_string()),
        organization_id: Some("org_updated".to_string()),
    });
    let updated_enabled = false;

    let update_payload = UpdateProviderRequest {
        name: Some(updated_name.clone()),
        config: Some(updated_config.clone()),
        enabled: Some(updated_enabled),
    };

    let update_response = client
        .put(&format!(
            "/api/v1/management/providers/{}",
            created_provider.id
        ))
        .json(&update_payload)
        .await;
    assert_eq!(
        update_response.status_code(),
        axum::http::StatusCode::OK,
        "Update provider request failed"
    );

    let updated_provider_response: ProviderResponse = update_response.json::<ProviderResponse>();

    assert_eq!(updated_provider_response.id, created_provider.id);
    assert_eq!(updated_provider_response.name, updated_name);
    assert_eq!(
        updated_provider_response.provider_type,
        created_provider.provider_type
    );
    assert_eq!(updated_provider_response.config, updated_config);
    assert_eq!(updated_provider_response.enabled, updated_enabled);

    assert_eq!(
        updated_provider_response.created_at,
        created_provider.created_at
    );
    assert!(
        updated_provider_response.updated_at > created_provider.updated_at,
        "Expected updated_at ({:?}) to be greater than original updated_at ({:?})",
        updated_provider_response.updated_at,
        created_provider.updated_at
    );
    assert!(updated_provider_response.updated_at >= updated_provider_response.created_at);

    let db_provider = sqlx::query_as!(
        Provider,
        r#"
            SELECT id, name, provider_type, config_details, enabled, created_at, updated_at
            FROM hub_llmgateway_providers
            WHERE id = $1
            "#,
        updated_provider_response.id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch provider from DB");
    assert_eq!(db_provider.name, updated_name);
    assert_eq!(db_provider.enabled, updated_enabled);
    let provider_type_enum: ProviderType = db_provider
        .provider_type
        .parse()
        .expect("Failed to parse provider_type from DB");
    let db_config = match provider_type_enum {
        ProviderType::OpenAI => {
            let config: OpenAIProviderConfig = serde_json::from_value(db_provider.config_details)
                .expect("Failed to deserialize OpenAI config from DB");
            ProviderConfig::OpenAI(config)
        }
        ProviderType::Azure => {
            let config: AzureProviderConfig = serde_json::from_value(db_provider.config_details)
                .expect("Failed to deserialize Azure config from DB");
            ProviderConfig::Azure(config)
        }
        ProviderType::Anthropic => {
            let config: AnthropicProviderConfig =
                serde_json::from_value(db_provider.config_details)
                    .expect("Failed to deserialize Anthropic config from DB");
            ProviderConfig::Anthropic(config)
        }
        ProviderType::Bedrock => {
            let config: BedrockProviderConfig = serde_json::from_value(db_provider.config_details)
                .expect("Failed to deserialize Bedrock config from DB");
            ProviderConfig::Bedrock(config)
        }
        ProviderType::VertexAI => {
            let config: VertexAIProviderConfig = serde_json::from_value(db_provider.config_details)
                .expect("Failed to deserialize VertexAI config from DB");
            ProviderConfig::VertexAI(config)
        }
    };
    assert_eq!(db_config, updated_config);
}

#[tokio::test]
async fn test_update_provider_not_found() {
    let (client, _pool, _container) = setup_test_environment().await;

    let non_existent_uuid = Uuid::new_v4();
    let update_payload = UpdateProviderRequest {
        name: Some("New Name for NonExistent".to_string()),
        config: None,
        enabled: Some(true),
    };

    let response = client
        .put(&format!(
            "/api/v1/management/providers/{}",
            non_existent_uuid
        ))
        .json(&update_payload)
        .await;

    assert_eq!(response.status_code(), axum::http::StatusCode::NOT_FOUND);
    let error_response: serde_json::Value = response.json::<serde_json::Value>();
    assert!(error_response["error"].as_str().unwrap().contains(&format!(
        "Provider with ID {} not found to update.",
        non_existent_uuid
    )));
}

#[tokio::test]
async fn test_update_provider_duplicate_name_conflict() {
    let (client, _pool, _container) = setup_test_environment().await;

    let provider1_name = "Name A - Original".to_string();
    let provider1_payload = CreateProviderRequest {
        name: provider1_name.clone(),
        provider_type: ProviderType::OpenAI,
        config: ProviderConfig::OpenAI(OpenAIProviderConfig {
            api_key: SecretObject::literal("key_A".to_string()),
            organization_id: None,
        }),
        enabled: Some(true),
    };
    let res1 = client
        .post("/api/v1/management/providers")
        .json(&provider1_payload)
        .await;
    assert_eq!(
        res1.status_code(),
        axum::http::StatusCode::CREATED,
        "Failed to create provider1 for conflict test"
    );

    let provider2_payload = CreateProviderRequest {
        name: "Name B - To Be Updated".to_string(),
        provider_type: ProviderType::Azure,
        config: ProviderConfig::Azure(AzureProviderConfig {
            api_key: SecretObject::literal("key_B".to_string()),
            resource_name: "resB".to_string(),
            api_version: "vB".to_string(),
            base_url: None,
        }),
        enabled: Some(true),
    };
    let res2 = client
        .post("/api/v1/management/providers")
        .json(&provider2_payload)
        .await;
    assert_eq!(
        res2.status_code(),
        axum::http::StatusCode::CREATED,
        "Failed to create provider2 for conflict test"
    );
    let provider2_created: ProviderResponse = res2.json::<ProviderResponse>();

    let update_payload_conflict = UpdateProviderRequest {
        name: Some(provider1_name.clone()),
        config: None,
        enabled: None,
    };

    let update_conflict_response = client
        .put(&format!(
            "/api/v1/management/providers/{}",
            provider2_created.id
        ))
        .json(&update_payload_conflict)
        .await;

    assert_eq!(
        update_conflict_response.status_code(),
        axum::http::StatusCode::CONFLICT
    );
    let error_response: serde_json::Value = update_conflict_response.json::<serde_json::Value>();
    assert!(error_response["error"].as_str().unwrap().contains(&format!(
        "Another provider with name '{}' already exists.",
        provider1_name
    )));
}

#[tokio::test]
async fn test_delete_provider_success() {
    let (client, pool, _container) = setup_test_environment().await;

    let provider_payload = CreateProviderRequest {
        name: "Provider To Delete".to_string(),
        provider_type: ProviderType::Azure,
        config: ProviderConfig::Azure(AzureProviderConfig {
            api_key: SecretObject::literal("delete_key".to_string()),
            resource_name: "del_res".to_string(),
            api_version: "del_v".to_string(),
            base_url: None,
        }),
        enabled: Some(true),
    };
    let create_res = client
        .post("/api/v1/management/providers")
        .json(&provider_payload)
        .await;
    assert_eq!(
        create_res.status_code(),
        axum::http::StatusCode::CREATED,
        "Failed to create provider for delete test"
    );
    let created_provider: ProviderResponse = create_res.json::<ProviderResponse>();

    let delete_response = client
        .delete(&format!(
            "/api/v1/management/providers/{}",
            created_provider.id
        ))
        .await;
    assert_eq!(
        delete_response.status_code(),
        axum::http::StatusCode::NO_CONTENT
    );

    let get_response_after_delete = client
        .get(&format!(
            "/api/v1/management/providers/{}",
            created_provider.id
        ))
        .await;
    assert_eq!(
        get_response_after_delete.status_code(),
        axum::http::StatusCode::NOT_FOUND
    );

    let db_provider_after_delete = sqlx::query_as!(
        Provider,
        r#"
            SELECT id, name, provider_type, config_details, enabled, created_at, updated_at
            FROM hub_llmgateway_providers
            WHERE id = $1
            "#,
        created_provider.id
    )
    .fetch_optional(&pool)
    .await
    .expect("DB query failed after delete");
    assert!(
        db_provider_after_delete.is_none(),
        "Provider should not exist in DB after delete"
    );
}

#[tokio::test]
async fn test_delete_provider_not_found() {
    let (client, _pool, _container) = setup_test_environment().await;

    let non_existent_uuid = Uuid::new_v4();

    let response = client
        .delete(&format!(
            "/api/v1/management/providers/{}",
            non_existent_uuid
        ))
        .await;

    assert_eq!(response.status_code(), axum::http::StatusCode::NOT_FOUND);

    let error_response: serde_json::Value = response.json::<serde_json::Value>();
    assert!(error_response["error"].as_str().unwrap().contains(&format!(
        "Provider with ID {} not found, nothing deleted.",
        non_existent_uuid
    )));
}

#[tokio::test]
async fn test_vertexai_provider_config_transformation() {
    let (client, _pool, _container) = setup_test_environment().await;

    let request_payload = CreateProviderRequest {
        name: "Test VertexAI Config Transform".to_string(),
        provider_type: ProviderType::VertexAI,
        config: ProviderConfig::VertexAI(VertexAIProviderConfig {
            project_id: Some("test-project-transform".to_string()),
            location: Some("us-central1".to_string()),
            credentials_path: Some("/path/to/credentials.json".to_string()),
            api_key: None,
        }),
        enabled: Some(true),
    };

    let response = client
        .post("/api/v1/management/providers")
        .json(&request_payload)
        .await;
    assert_eq!(response.status_code(), axum::http::StatusCode::CREATED);

    let provider_response: ProviderResponse = response.json::<ProviderResponse>();

    // Verify the configuration was stored and retrieved correctly
    if let ProviderConfig::VertexAI(config) = provider_response.config {
        assert_eq!(
            config.project_id,
            Some("test-project-transform".to_string())
        );
        assert_eq!(config.location, Some("us-central1".to_string()));
        assert_eq!(
            config.credentials_path,
            Some("/path/to/credentials.json".to_string())
        );
        assert_eq!(config.api_key, None);
    } else {
        panic!(
            "Expected VertexAI config, got {:?}",
            provider_response.config
        );
    }
}

#[tokio::test]
async fn test_create_anthropic_provider_success() {
    let (client, _pool, _container) = setup_test_environment().await;

    let request_payload = CreateProviderRequest {
        name: "Test Anthropic Provider".to_string(),
        provider_type: ProviderType::Anthropic,
        config: ProviderConfig::Anthropic(AnthropicProviderConfig {
            api_key: SecretObject::literal("test_anthropic_key".to_string()),
        }),
        enabled: Some(true),
    };

    let response = client
        .post("/api/v1/management/providers")
        .json(&request_payload)
        .await;

    assert_eq!(response.status_code(), axum::http::StatusCode::CREATED);

    let provider_response: ProviderResponse = response.json::<ProviderResponse>();

    assert_eq!(provider_response.name, request_payload.name);
    assert_eq!(provider_response.provider_type, ProviderType::Anthropic);
    assert_eq!(provider_response.enabled, true);

    // Verify the configuration was stored correctly
    if let ProviderConfig::Anthropic(config) = provider_response.config {
        assert_eq!(
            config.api_key,
            SecretObject::literal("test_anthropic_key".to_string())
        );
    } else {
        panic!(
            "Expected Anthropic config, got {:?}",
            provider_response.config
        );
    }
}
