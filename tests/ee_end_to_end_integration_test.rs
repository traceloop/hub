#[cfg(feature = "ee_feature")]
mod ee_integration_tests {
    use serde_json::json;
    use sqlx::PgPool;
    use std::process::{Command, Stdio};
    use std::time::Duration;
    use testcontainers::{runners::AsyncRunner, ContainerAsync};
    use testcontainers_modules::postgres::Postgres;
    use tokio::process::Child;
    use tokio::time::sleep;

    struct TestEnvironment {
        _postgres_container: ContainerAsync<Postgres>,
        pool: PgPool,
        _app_process: Child,
        client: reqwest::Client,
        base_url: String,
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
            sqlx::migrate!("ee/migrations").run(&pool).await?;

            // Find an available port for the test server
            let port = find_available_port().await?;
            let base_url = format!("http://127.0.0.1:{}", port);

            // Set environment variables for the application
            std::env::set_var("DATABASE_URL", &connection_string);
            std::env::set_var("PORT", &port.to_string());
            std::env::set_var("DB_POLL_INTERVAL_SECONDS", "1"); // Fast polling for tests

            // Build the application binary
            let build_output = Command::new("cargo")
                .args(&["build", "--features", "ee_feature"])
                .output()?;

            if !build_output.status.success() {
                anyhow::bail!(
                    "Failed to build application: {}",
                    String::from_utf8_lossy(&build_output.stderr)
                );
            }

            // Start the application process
            let app_process = tokio::process::Command::new("./target/debug/hub")
                .env("DATABASE_URL", &connection_string)
                .env("PORT", &port.to_string())
                .env("DB_POLL_INTERVAL_SECONDS", "1")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            // Wait for the application to start
            let client = reqwest::Client::new();
            let health_url = format!("{}/health", base_url);

            // Wait up to 10 seconds for the app to start
            for _ in 0..50 {
                if let Ok(response) = client.get(&health_url).send().await {
                    if response.status().is_success() {
                        println!("✓ Application started successfully on port {}", port);
                        break;
                    }
                }
                sleep(Duration::from_millis(200)).await;
            }

            // Verify the app is responding
            let response = client.get(&health_url).send().await?;
            if !response.status().is_success() {
                anyhow::bail!("Application failed to start properly");
            }

            Ok(TestEnvironment {
                _postgres_container: postgres_container,
                pool,
                _app_process: app_process,
                client,
                base_url,
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
                    "api_key": api_key,
                    "organization_id": null
                }
            });

            let response = self
                .client
                .post(&format!("{}/ee/api/v1/providers", self.base_url))
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
                .post(&format!("{}/ee/api/v1/model-definitions", self.base_url))
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
                .post(&format!("{}/ee/api/v1/pipelines", self.base_url))
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
                .post(&format!("{}/api/v1/chat/completions", self.base_url))
                .header("content-type", "application/json")
                .json(&request)
                .send()
                .await?;

            Ok(response)
        }
    }

    impl Drop for TestEnvironment {
        fn drop(&mut self) {
            // The Child process will be automatically killed when dropped
        }
    }

    async fn find_available_port() -> anyhow::Result<u16> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        drop(listener); // Release the port
        Ok(port)
    }

    #[tokio::test]
    async fn test_end_to_end_ee_integration() {
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

        // Step 2: Create a provider
        println!("Step 2: Creating OpenAI provider...");
        let provider = env
            .create_provider("openai-provider", "test-api-key")
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

        // Step 6: Try the same request again - should now route to provider (but fail with auth)
        println!("Step 6: Testing request with configuration...");
        let response = env
            .make_chat_request("gpt-3.5-turbo")
            .await
            .expect("Request failed");

        // Should now find the pipeline and route to provider, but fail with 401 due to invalid API key
        assert!(
            response.status() == 401 || response.status() == 500,
            "Expected 401 or 500 when routing to provider with invalid API key, got {}",
            response.status()
        );
        println!("✓ Request correctly routed to provider and failed with auth error");

        // Step 7: Verify the configuration is in the database
        println!("Step 7: Verifying database state...");

        let provider_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_ee_providers")
                .fetch_one(&env.pool)
                .await
                .expect("Failed to count providers");
        assert_eq!(provider_count, 1, "Expected 1 provider in database");

        let model_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_ee_model_definitions")
                .fetch_one(&env.pool)
                .await
                .expect("Failed to count models");
        assert_eq!(model_count, 1, "Expected 1 model in database");

        let pipeline_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hub_llmgateway_ee_pipelines")
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
            .post(&format!("{}/ee/api/v1/model-definitions", env.base_url))
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
            .post(&format!("{}/ee/api/v1/pipelines", env.base_url))
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
            .post(&format!("{}/ee/api/v1/pipelines", env.base_url))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 201, "Expected 201 for valid pipeline");
        println!("✓ Valid pipeline correctly created");

        println!("✓ Configuration validation test completed successfully!");
    }
}

#[cfg(not(feature = "ee_feature"))]
mod ee_integration_tests {
    #[tokio::test]
    async fn test_ee_features_disabled() {
        println!("EE features are disabled - skipping EE integration tests");
        // This test always passes when EE features are disabled
    }
}
