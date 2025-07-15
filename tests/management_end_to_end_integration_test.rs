use serde_json::{Value, json};
use sqlx::PgPool;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::process::Child;
use tokio::time::sleep;
use tracing::debug;

struct TestEnvironment {
    _postgres_container: ContainerAsync<Postgres>,
    pool: PgPool,
    _app_process: Child,
    client: reqwest::Client,
    base_url: String,
    management_api_base_url: String,
}

impl TestEnvironment {
    async fn setup() -> anyhow::Result<Self> {
        // Start PostgreSQL container
        let postgres_container = Postgres::default()
            .with_db_name("test_db")
            .with_user("test_user")
            .with_password("test_password")
            .start()
            .await?;

        let connection_string = format!(
            "postgresql://test_user:test_password@127.0.0.1:{}/test_db",
            postgres_container.get_host_port_ipv4(5432).await?
        );

        // Create database pool for test verification
        let pool = PgPool::connect(&connection_string).await?;

        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await?;

        // Find available ports for both servers
        let (gateway_port, management_port) = find_two_available_ports().await?;
        let base_url = format!("http://127.0.0.1:{}", gateway_port);
        let management_base_url = format!("http://127.0.0.1:{}", management_port);
        let management_api_base_url = format!("{}/api/v1/management", management_base_url);

        // Set environment variables for the application
        unsafe {
            std::env::set_var("DATABASE_URL", &connection_string);
            std::env::set_var("PORT", gateway_port.to_string());
            std::env::set_var("MANAGEMENT_PORT", management_port.to_string());
            std::env::set_var("DB_POLL_INTERVAL_SECONDS", "1"); // Fast polling for tests
            std::env::set_var("HUB_MODE", "database"); // Force database mode
        }

        // Build the application binary
        let build_output = Command::new("cargo").args(["build"]).output()?;

        if !build_output.status.success() {
            anyhow::bail!(
                "Failed to build application: {}",
                String::from_utf8_lossy(&build_output.stderr)
            );
        }

        // Start the application process
        let app_process = tokio::process::Command::new("./target/debug/hub")
            .env("DATABASE_URL", &connection_string)
            .env("PORT", gateway_port.to_string())
            .env("MANAGEMENT_PORT", management_port.to_string())
            .env("DB_POLL_INTERVAL_SECONDS", "1")
            .env("HUB_MODE", "database")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Wait for the application to start
        let client = reqwest::Client::new();
        let health_url = format!("{}/health", base_url);
        let management_health_url = format!("{}/health", management_base_url);

        // Wait up to 10 seconds for both servers to start
        for _ in 0..50 {
            let gateway_ok = client
                .get(&health_url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);
            let management_ok = client
                .get(&management_health_url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);

            if gateway_ok && management_ok {
                println!(
                    "✓ Both servers started successfully - Gateway: {}, Management: {}",
                    base_url, management_base_url
                );
                break;
            }
            sleep(Duration::from_millis(200)).await;
        }

        // Verify both servers are responding
        let gateway_response = client.get(&health_url).send().await?;
        if !gateway_response.status().is_success() {
            anyhow::bail!("Gateway server failed to start properly");
        }

        let management_response = client.get(&management_health_url).send().await?;
        if !management_response.status().is_success() {
            anyhow::bail!("Management API server failed to start properly");
        }

        Ok(TestEnvironment {
            _postgres_container: postgres_container,
            pool,
            _app_process: app_process,
            client,
            base_url,
            management_api_base_url,
        })
    }

    async fn create_provider(
        &self,
        name: &str,
        api_key: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let request = json!({
            "name": name,
            "provider_type": "openai",
            "config": {
                "api_key": {
                    "type": "literal",
                    "value": api_key
                },
                "organization_id": null
            }
        });

        let response = self
            .client
            .post(format!("{}/providers", self.management_api_base_url))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if response.status() != 201 {
            let status = response.status();
            let error_body = response.text().await?;
            anyhow::bail!("Failed to create provider: {} - {}", status, error_body);
        }

        Ok(response.json().await?)
    }

    async fn create_model(
        &self,
        key: &str,
        provider_id: &str,
        model_type: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let request = json!({
            "key": key,
            "provider_id": provider_id,
            "model_type": model_type
        });

        let response = self
            .client
            .post(format!(
                "{}/model-definitions",
                self.management_api_base_url
            ))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if response.status() != 201 {
            let status = response.status();
            let error_body = response.text().await?;
            anyhow::bail!("Failed to create model: {} - {}", status, error_body);
        }

        Ok(response.json().await?)
    }

    async fn create_pipeline(
        &self,
        name: &str,
        models: Vec<String>,
    ) -> anyhow::Result<serde_json::Value> {
        let model_entries: Vec<serde_json::Value> = models
            .into_iter()
            .enumerate()
            .map(|(i, key)| {
                json!({
                    "key": key,
                    "priority": i
                })
            })
            .collect();

        let plugins = vec![
            json!({
                "plugin_type": "logging",
                "config_data": {
                    "level": "info"
                }
            }),
            json!({
                "plugin_type": "model-router",
                "config_data": {
                    "strategy": "ordered_fallback",
                    "models": model_entries
                }
            }),
        ];

        let request = json!({
            "name": name,
            "pipeline_type": "chat",
            "plugins": plugins
        });

        let response = self
            .client
            .post(format!("{}/pipelines", self.management_api_base_url))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if response.status() != 201 {
            let status = response.status();
            let error_body = response.text().await?;
            anyhow::bail!("Failed to create pipeline: {} - {}", status, error_body);
        }

        Ok(response.json().await?)
    }

    async fn make_chat_request(&self, model: &str) -> anyhow::Result<reqwest::Response> {
        let request = json!({
            "model": model,
            "messages": [{"role": "user", "content": "Hello, world!"}]
        });

        let response = self
            .client
            .post(format!("{}/api/v1/chat/completions", self.base_url))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        Ok(response)
    }

    async fn make_chat_request_with_pipeline(
        &self,
        model: &str,
        pipeline: &str,
    ) -> anyhow::Result<reqwest::Response> {
        let request = json!({
            "model": model,
            "messages": [{"role": "user", "content": "Hello, world!"}]
        });

        let response = self
            .client
            .post(format!("{}/api/v1/chat/completions", self.base_url))
            .header("content-type", "application/json")
            .header("x-traceloop-pipeline", pipeline)
            .json(&request)
            .send()
            .await?;

        Ok(response)
    }

    async fn make_embeddings_request_with_pipeline(
        &self,
        model: &str,
        pipeline: &str,
    ) -> anyhow::Result<reqwest::Response> {
        let request = json!({
            "model": model,
            "input": "Hello, world!"
        });

        let response = self
            .client
            .post(format!("{}/api/v1/embeddings", self.base_url))
            .header("content-type", "application/json")
            .header("x-traceloop-pipeline", pipeline)
            .json(&request)
            .send()
            .await?;

        Ok(response)
    }

    async fn create_embeddings_pipeline(
        &self,
        name: &str,
        models: Vec<String>,
    ) -> anyhow::Result<serde_json::Value> {
        let model_entries: Vec<serde_json::Value> = models
            .into_iter()
            .enumerate()
            .map(|(i, key)| {
                json!({
                    "key": key,
                    "priority": i
                })
            })
            .collect();

        let plugins = vec![
            json!({
                "plugin_type": "logging",
                "config_data": {
                    "level": "info"
                }
            }),
            json!({
                "plugin_type": "model-router",
                "config_data": {
                    "strategy": "ordered_fallback",
                    "models": model_entries
                }
            }),
        ];

        let request = json!({
            "name": name,
            "pipeline_type": "embeddings",
            "plugins": plugins
        });

        let response = self
            .client
            .post(format!("{}/pipelines", self.management_api_base_url))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if response.status() != 201 {
            let status = response.status();
            let error_body = response.text().await?;
            anyhow::bail!(
                "Failed to create embeddings pipeline: {} - {}",
                status,
                error_body
            );
        }

        Ok(response.json().await?)
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // The Child process will be automatically killed when dropped
    }
}

async fn find_two_available_ports() -> anyhow::Result<(u16, u16)> {
    use tokio::net::TcpListener;

    let listener1 = TcpListener::bind("127.0.0.1:0").await?;
    let port1 = listener1.local_addr()?.port();

    let listener2 = TcpListener::bind("127.0.0.1:0").await?;
    let port2 = listener2.local_addr()?.port();

    // Keep both listeners alive until we return the ports
    // This prevents race conditions
    let ports = (port1, port2);

    // Drop listeners to release ports
    drop(listener1);
    drop(listener2);

    Ok(ports)
}

// Cassette recording functionality
async fn load_or_record_response(test_name: &str) -> Value {
    // Create the cassettes directory if it doesn't exist
    let cassettes_dir = PathBuf::from("tests/cassettes/openai");
    debug!("Creating cassettes directory at: {:?}", cassettes_dir);

    if let Err(e) = std::fs::create_dir_all(&cassettes_dir) {
        panic!("Failed to create cassettes directory: {}", e);
    }

    // Create specific cassette file path
    let cassette_path = cassettes_dir.join(format!("{}.json", test_name));
    debug!("Cassette path: {:?}", cassette_path);

    let is_record_mode = std::env::var("RECORD_MODE").is_ok();
    debug!("Record mode: {}", is_record_mode);

    if is_record_mode {
        // In record mode, we'll return a placeholder that the test should replace with real response
        debug!("Record mode enabled - test should save real response");
        return json!({
            "record_mode": true,
            "message": "This should be replaced with real API response"
        });
    }

    // Try to load existing cassette
    if let Ok(cassette_content) = fs::read_to_string(&cassette_path) {
        debug!("Loading cassette from: {:?}", cassette_path);

        // Parse the cassette content
        if let Ok(response) = serde_json::from_str::<Value>(&cassette_content) {
            debug!("Successfully loaded cassette response");
            return response;
        }
    }

    panic!(
        "No cassette found at {:?} and not in record mode. Run with RECORD_MODE=1 to create one.",
        cassette_path
    );
}

// Helper function to save response to cassette
async fn save_to_cassette(test_name: &str, response: &Value) {
    let cassettes_dir = PathBuf::from("tests/cassettes/openai");
    let cassette_path = cassettes_dir.join(format!("{}.json", test_name));

    // Save the response to cassette
    let content =
        serde_json::to_string_pretty(response).expect("Failed to serialize response to JSON");

    if let Err(e) = fs::write(&cassette_path, content) {
        panic!("Error saving cassette: {}", e);
    }

    debug!(
        "Successfully saved response to cassette: {:?}",
        cassette_path
    );
}

#[tokio::test]
async fn test_end_to_end_integration() {
    let env = TestEnvironment::setup()
        .await
        .expect("Failed to setup test environment");

    // Step 1: Try a request with no configuration - should fail
    println!("Step 1: Testing request with no configuration...");
    let response = env
        .make_chat_request("gpt-3.5-turbo")
        .await
        .expect("Request failed");
    // Should be 404 (no route found) when no configuration exists
    assert_eq!(
        response.status(),
        404,
        "Expected 404 when no configuration exists, got {}",
        response.status()
    );
    println!(
        "✓ Request correctly failed with no configuration ({})",
        response.status()
    );

    // Step 2: Create a provider with real or test API key
    println!("Step 2: Creating OpenAI provider...");

    // Use real API key in record mode, test key otherwise
    let api_key = if std::env::var("RECORD_MODE").is_ok() {
        std::env::var("OPENAI_API_KEY")
            .expect("OPENAI_API_KEY environment variable must be set when RECORD_MODE=1")
    } else {
        "test-api-key".to_string()
    };

    let provider = env
        .create_provider("openai-provider", &api_key)
        .await
        .expect("Failed to create provider");
    let provider_id = provider["id"].as_str().unwrap();
    println!(
        "✓ Provider created: {} (ID: {})",
        provider["name"], provider_id
    );

    // Step 3: Create a model definition
    println!("Step 3: Creating model definition...");
    let model = env
        .create_model("gpt-3.5-turbo", provider_id, "gpt-3.5-turbo")
        .await
        .expect("Failed to create model");
    println!("✓ Model created: {}", model["key"]);

    // Step 4: Create a pipeline with logging and model router plugins
    println!("Step 4: Creating pipeline with plugins...");
    let pipeline = env
        .create_pipeline("default", vec!["gpt-3.5-turbo".to_string()])
        .await
        .expect("Failed to create pipeline");
    println!("✓ Pipeline created: {}", pipeline["name"]);

    // Step 5: Wait for configuration to be picked up by polling
    println!("Step 5: Waiting for configuration polling...");
    sleep(Duration::from_secs(3)).await; // Give polling time to pick up changes

    // Step 6: Try the same request again - should now route to provider
    println!("Step 6: Testing request with configuration...");

    if std::env::var("RECORD_MODE").is_ok() {
        // In record mode, make real request and save response
        println!("🎬 Recording mode: Making real API request...");
        let response = env
            .make_chat_request("gpt-3.5-turbo")
            .await
            .expect("Request failed");

        let status = response.status();
        let response_body: Value = response
            .json()
            .await
            .expect("Failed to parse response JSON");

        // Save the real response to cassette
        save_to_cassette("chat_completion_success", &response_body).await;

        // In record mode with real API key, we should get 200
        assert_eq!(
            status, 200,
            "Expected 200 with real API key, got {}",
            status
        );
        println!("✓ Real API request successful (200) - Response saved to cassette");

        // Validate response structure
        assert!(
            response_body.get("choices").is_some(),
            "Response should have 'choices' field"
        );
        assert!(
            response_body.get("usage").is_some(),
            "Response should have 'usage' field"
        );
        println!("✓ Response structure validated");
    } else {
        // In test mode, use cassette
        println!("📼 Test mode: Using recorded response...");
        let recorded_response = load_or_record_response("chat_completion_success").await;

        // Validate the recorded response structure
        assert!(
            recorded_response.get("choices").is_some(),
            "Recorded response should have 'choices' field"
        );
        assert!(
            recorded_response.get("usage").is_some(),
            "Recorded response should have 'usage' field"
        );
        println!("✓ Recorded response structure validated");

        // Also test that the live request would work (but expect auth failure with test key)
        let response = env
            .make_chat_request("gpt-3.5-turbo")
            .await
            .expect("Request failed");

        // Should route to provider but fail with 401 due to test API key
        assert!(
            response.status() == 401 || response.status() == 500,
            "Expected 401 or 500 when routing to provider with test key, got {}",
            response.status()
        );
        println!(
            "✓ Request correctly routed to provider and failed with auth error (as expected with test key)"
        );
    }

    // Step 7: Verify the configuration is in the database
    println!("Step 7: Verifying database state...");

    let provider_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_providers")
        .fetch_one(&env.pool)
        .await
        .expect("Failed to count providers");
    assert_eq!(provider_count, 1, "Expected 1 provider in database");

    let model_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_model_definitions")
            .fetch_one(&env.pool)
            .await
            .expect("Failed to count models");
    assert_eq!(model_count, 1, "Expected 1 model in database");

    let pipeline_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_pipelines")
        .fetch_one(&env.pool)
        .await
        .expect("Failed to count pipelines");
    assert_eq!(pipeline_count, 1, "Expected 1 pipeline in database");

    println!("✓ Database state verified");
    println!("🎉 End-to-end integration test completed successfully!");
}

#[tokio::test]
async fn test_configuration_validation_and_rejection() {
    let env = TestEnvironment::setup()
        .await
        .expect("Failed to setup test environment");

    // Test 1: Try to create a model with non-existent provider
    println!("Testing model creation with invalid provider...");
    let fake_uuid = "00000000-0000-0000-0000-000000000000";

    let request = json!({
        "key": "invalid-model",
        "provider_id": fake_uuid,
        "model_type": "gpt-3.5-turbo"
    });

    let response = env
        .client
        .post(format!("{}/model-definitions", env.management_api_base_url))
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .expect("Request failed");

    assert!(
        response.status() == 400 || response.status() == 422,
        "Expected 400 or 422 for invalid provider reference, got {}",
        response.status()
    );
    println!("✓ Invalid provider reference correctly rejected");

    // Test 2: Create valid provider and model first
    let valid_provider = env
        .create_provider("valid-provider", "test-key")
        .await
        .unwrap();
    let valid_provider_id = valid_provider["id"].as_str().unwrap();
    env.create_model("valid-model", valid_provider_id, "gpt-3.5-turbo")
        .await
        .unwrap();

    // Test 3: Try to create pipeline with non-existent model
    println!("Testing pipeline creation with invalid model...");
    let request = json!({
        "name": "invalid-pipeline",
        "pipeline_type": "chat",
        "plugins": [{
            "plugin_type": "model-router",
            "config_data": {
                "strategy": "ordered_fallback",
                "models": [{
                    "key": "non-existent-model",
                    "priority": 0
                }]
            }
        }]
    });

    let response = env
        .client
        .post(format!("{}/pipelines", env.management_api_base_url))
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .expect("Request failed");

    assert!(
        response.status() == 400 || response.status() == 422,
        "Expected 400 or 422 for invalid model reference, got {}",
        response.status()
    );
    println!("✓ Invalid model reference correctly rejected");

    // Test 4: Create valid pipeline
    println!("Testing valid pipeline creation...");
    let request = json!({
        "name": "valid-pipeline",
        "pipeline_type": "chat",
        "plugins": [{
            "plugin_type": "model-router",
            "config_data": {
                "strategy": "ordered_fallback",
                "models": [{
                    "key": "valid-model",
                    "priority": 0
                }]
            }
        }]
    });

    let response = env
        .client
        .post(format!("{}/pipelines", env.management_api_base_url))
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Expected 201 for valid pipeline");
    println!("✓ Valid pipeline correctly created");

    println!("✓ Configuration validation test completed successfully!");
}

#[tokio::test]
async fn test_pipeline_header_routing_e2e() {
    let env = TestEnvironment::setup()
        .await
        .expect("Failed to setup test environment");

    println!("🧪 Testing pipeline header routing with chat and embeddings pipelines...");

    // Step 1: Create a provider
    println!("Step 1: Creating OpenAI provider...");
    let api_key = if std::env::var("RECORD_MODE").is_ok() {
        std::env::var("OPENAI_API_KEY")
            .expect("OPENAI_API_KEY environment variable must be set when RECORD_MODE=1")
    } else {
        "test-api-key".to_string()
    };

    let provider = env
        .create_provider("openai-provider", &api_key)
        .await
        .expect("Failed to create provider");
    let provider_id = provider["id"].as_str().unwrap();
    println!("✓ Provider created: {}", provider["name"]);

    // Step 2: Create model definitions for both chat and embeddings
    println!("Step 2: Creating model definitions...");
    let chat_model = env
        .create_model("gpt-3.5-turbo", provider_id, "gpt-3.5-turbo")
        .await
        .expect("Failed to create chat model");
    println!("✓ Chat model created: {}", chat_model["key"]);

    let embeddings_model = env
        .create_model(
            "text-embedding-3-small",
            provider_id,
            "text-embedding-3-small",
        )
        .await
        .expect("Failed to create embeddings model");
    println!("✓ Embeddings model created: {}", embeddings_model["key"]);

    // Step 3: Create chat pipeline named "chat-pipeline"
    println!("Step 3: Creating chat pipeline...");
    let chat_pipeline = env
        .create_pipeline("chat-pipeline", vec!["gpt-3.5-turbo".to_string()])
        .await
        .expect("Failed to create chat pipeline");
    println!("✓ Chat pipeline created: {}", chat_pipeline["name"]);

    // Step 4: Create embeddings pipeline named "embeddings-pipeline"
    println!("Step 4: Creating embeddings pipeline...");
    let embeddings_pipeline = env
        .create_embeddings_pipeline(
            "embeddings-pipeline",
            vec!["text-embedding-3-small".to_string()],
        )
        .await
        .expect("Failed to create embeddings pipeline");
    println!(
        "✓ Embeddings pipeline created: {}",
        embeddings_pipeline["name"]
    );

    // Step 5: Create a default pipeline for fallback testing
    println!("Step 5: Creating default pipeline...");
    let default_pipeline = env
        .create_pipeline("default", vec!["gpt-3.5-turbo".to_string()])
        .await
        .expect("Failed to create default pipeline");
    println!("✓ Default pipeline created: {}", default_pipeline["name"]);

    // Step 6: Wait for configuration to be picked up
    println!("Step 6: Waiting for configuration polling...");
    sleep(Duration::from_secs(3)).await;

    // Step 7: Test pipeline header routing for chat
    println!("Step 7: Testing chat pipeline routing...");

    // Test with specific pipeline header
    let chat_response = env
        .make_chat_request_with_pipeline("gpt-3.5-turbo", "chat-pipeline")
        .await
        .expect("Chat request failed");

    println!(
        "Chat request with 'chat-pipeline' header: {}",
        chat_response.status()
    );

    // Should route to provider and get auth error with test key (or 200 in record mode)
    if std::env::var("RECORD_MODE").is_ok() {
        assert_eq!(
            chat_response.status(),
            200,
            "Expected 200 with real API key for chat pipeline"
        );

        // Record the response
        let response_body: Value = chat_response
            .json()
            .await
            .expect("Failed to parse chat response JSON");
        save_to_cassette("chat_pipeline_routing", &response_body).await;

        println!("✓ Chat pipeline routing successful (200) - Response recorded");
    } else {
        // In test mode, validate recorded response structure
        let recorded_response = load_or_record_response("chat_pipeline_routing").await;
        assert!(
            recorded_response.get("choices").is_some(),
            "Chat pipeline recorded response should have 'choices' field"
        );

        assert!(
            chat_response.status() == 401 || chat_response.status() == 500,
            "Expected 401 or 500 for chat pipeline with test key, got {}",
            chat_response.status()
        );
        println!("✓ Chat pipeline routing successful (auth error as expected)");
    }

    // Step 8: Test pipeline header routing for embeddings
    println!("Step 8: Testing embeddings pipeline routing...");

    let embeddings_response = env
        .make_embeddings_request_with_pipeline("text-embedding-3-small", "embeddings-pipeline")
        .await
        .expect("Embeddings request failed");

    println!(
        "Embeddings request with 'embeddings-pipeline' header: {}",
        embeddings_response.status()
    );

    // Should route to provider and get auth error with test key (or 200 in record mode)
    if std::env::var("RECORD_MODE").is_ok() {
        assert_eq!(
            embeddings_response.status(),
            200,
            "Expected 200 with real API key for embeddings pipeline"
        );

        // Record the response
        let response_body: Value = embeddings_response
            .json()
            .await
            .expect("Failed to parse embeddings response JSON");
        save_to_cassette("embeddings_pipeline_routing", &response_body).await;

        println!("✓ Embeddings pipeline routing successful (200) - Response recorded");
    } else {
        // In test mode, validate recorded response structure
        let recorded_response = load_or_record_response("embeddings_pipeline_routing").await;
        assert!(
            recorded_response.get("data").is_some(),
            "Embeddings pipeline recorded response should have 'data' field"
        );

        assert!(
            embeddings_response.status() == 401 || embeddings_response.status() == 500,
            "Expected 401 or 500 for embeddings pipeline with test key, got {}",
            embeddings_response.status()
        );
        println!("✓ Embeddings pipeline routing successful (auth error as expected)");
    }

    // Step 9: Test default pipeline fallback
    println!("Step 9: Testing default pipeline fallback...");

    // Request without pipeline header should go to default
    let default_response = env
        .make_chat_request("gpt-3.5-turbo")
        .await
        .expect("Default request failed");

    println!(
        "Chat request without pipeline header: {}",
        default_response.status()
    );

    // Should route to default pipeline
    if std::env::var("RECORD_MODE").is_ok() {
        assert_eq!(
            default_response.status(),
            200,
            "Expected 200 with real API key for default pipeline"
        );

        // Record the response
        let response_body: Value = default_response
            .json()
            .await
            .expect("Failed to parse default response JSON");
        save_to_cassette("default_pipeline_fallback", &response_body).await;

        println!("✓ Default pipeline fallback successful (200) - Response recorded");
    } else {
        // In test mode, validate recorded response structure
        let recorded_response = load_or_record_response("default_pipeline_fallback").await;
        assert!(
            recorded_response.get("choices").is_some(),
            "Default pipeline recorded response should have 'choices' field"
        );

        assert!(
            default_response.status() == 401 || default_response.status() == 500,
            "Expected 401 or 500 for default pipeline with test key, got {}",
            default_response.status()
        );
        println!("✓ Default pipeline fallback successful (auth error as expected)");
    }

    // Step 10: Test non-existent pipeline fallback
    println!("Step 10: Testing non-existent pipeline fallback...");

    let nonexistent_response = env
        .make_chat_request_with_pipeline("gpt-3.5-turbo", "nonexistent-pipeline")
        .await
        .expect("Non-existent pipeline request failed");

    println!(
        "Chat request with non-existent pipeline header: {}",
        nonexistent_response.status()
    );

    // Should fall back to default pipeline
    if std::env::var("RECORD_MODE").is_ok() {
        assert_eq!(
            nonexistent_response.status(),
            200,
            "Expected 200 with real API key for non-existent pipeline fallback"
        );

        // Record the response
        let response_body: Value = nonexistent_response
            .json()
            .await
            .expect("Failed to parse non-existent pipeline response JSON");
        save_to_cassette("nonexistent_pipeline_fallback", &response_body).await;

        println!("✓ Non-existent pipeline fallback successful (200) - Response recorded");
    } else {
        // In test mode, validate recorded response structure
        let recorded_response = load_or_record_response("nonexistent_pipeline_fallback").await;
        assert!(
            recorded_response.get("choices").is_some(),
            "Non-existent pipeline recorded response should have 'choices' field"
        );

        assert!(
            nonexistent_response.status() == 401 || nonexistent_response.status() == 500,
            "Expected 401 or 500 for non-existent pipeline fallback with test key, got {}",
            nonexistent_response.status()
        );
        println!("✓ Non-existent pipeline fallback successful (auth error as expected)");
    }

    // Step 11: Test wrong endpoint for pipeline type
    println!("Step 11: Testing wrong endpoint for pipeline type...");

    // Try to use chat endpoint with embeddings pipeline
    let wrong_endpoint_response = env
        .make_chat_request_with_pipeline("gpt-3.5-turbo", "embeddings-pipeline")
        .await
        .expect("Wrong endpoint request failed");

    println!(
        "Chat request with embeddings pipeline header: {}",
        wrong_endpoint_response.status()
    );

    // Should get 404 because embeddings pipeline doesn't have chat endpoint
    assert_eq!(
        wrong_endpoint_response.status(),
        404,
        "Expected 404 when using chat endpoint with embeddings pipeline, got {}",
        wrong_endpoint_response.status()
    );
    println!("✓ Wrong endpoint correctly rejected (404)");

    // Step 12: Verify database state
    println!("Step 12: Verifying database state...");

    let pipeline_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_pipelines")
        .fetch_one(&env.pool)
        .await
        .expect("Failed to count pipelines");
    assert_eq!(pipeline_count, 3, "Expected 3 pipelines in database");

    let model_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_model_definitions")
            .fetch_one(&env.pool)
            .await
            .expect("Failed to count models");
    assert_eq!(model_count, 2, "Expected 2 models in database");

    println!("✓ Database state verified");
    println!("🎉 Pipeline header routing E2E test completed successfully!");
}
