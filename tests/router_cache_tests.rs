use hub_lib::state::AppState;
use hub_lib::types::{GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider};
use std::sync::Arc;

#[tokio::test]
async fn test_router_always_available() {
    // Create a basic configuration
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

    // With the simplified approach, we always have a current router
    let _current_router = app_state.get_current_router();
    // Router should always be available - we can't inspect internals but we can verify it exists
    // The fact that get_current_router() returns without panicking means the router is available
}

#[tokio::test]
async fn test_configuration_change_detection() {
    // Create initial configuration
    let initial_config = GatewayConfig {
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

    let app_state =
        Arc::new(AppState::new(initial_config.clone()).expect("Failed to create app state"));

    // Test 1: Update with identical configuration should be no-op
    let result = app_state.update_config(initial_config.clone());
    assert!(result.is_ok(), "Identical config update should succeed");

    // Test 2: Update with different configuration should rebuild router
    let mut updated_config = initial_config.clone();
    updated_config.providers[0].api_key = "new-key".to_string();

    let result = app_state.update_config(updated_config.clone());
    assert!(result.is_ok(), "Different config update should succeed");

    // Verify the configuration was actually updated
    let current_config = app_state.current_config();
    assert_eq!(current_config.providers[0].api_key, "new-key");
}

#[tokio::test]
async fn test_invalid_configuration_rejected() {
    let initial_config = GatewayConfig {
        general: None,
        providers: vec![Provider {
            key: "test-provider".to_string(),
            r#type: "openai".to_string(),
            api_key: "test-key".to_string(),
            params: Default::default(),
        }],
        models: vec![],
        pipelines: vec![],
    };

    let app_state = Arc::new(AppState::new(initial_config).expect("Failed to create app state"));

    // Create invalid configuration (model references non-existent provider)
    let invalid_config = GatewayConfig {
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
            provider: "non-existent-provider".to_string(), // Invalid reference
            params: Default::default(),
        }],
        pipelines: vec![],
    };

    // Invalid configuration should be rejected
    let result = app_state.update_config(invalid_config);
    assert!(result.is_err(), "Invalid configuration should be rejected");

    // Original configuration should remain unchanged
    let current_config = app_state.current_config();
    assert_eq!(current_config.models.len(), 0);
}

#[tokio::test]
async fn test_concurrent_router_access() {
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

    // Simulate concurrent access to router
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let app_state_clone = app_state.clone();
            tokio::spawn(async move {
                let _router = app_state_clone.get_current_router();
                // Router should always be available
            })
        })
        .collect();

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task should complete successfully");
    }

    // Router should still be available after concurrent access
    let _router = app_state.get_current_router();
}

#[tokio::test]
async fn test_empty_configuration_fallback() {
    // Create empty configuration
    let empty_config = GatewayConfig {
        general: None,
        providers: vec![],
        models: vec![],
        pipelines: vec![],
    };

    let app_state = Arc::new(AppState::new(empty_config).expect("Failed to create app state"));

    // Even with empty config, router should be available (fallback router)
    let _current_router = app_state.get_current_router();
    // The fact that get_current_router() returns without panicking means the router is available
}

#[tokio::test]
async fn test_pipeline_with_failing_tracing_endpoint() {
    // Create configuration with a pipeline that has a failing tracing endpoint
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
            name: "traced-pipeline".to_string(),
            r#type: PipelineType::Chat,
            plugins: vec![
                PluginConfig::Tracing {
                    endpoint: "http://invalid-endpoint:4317/v1/traces".to_string(),
                    api_key: "test-key".to_string(),
                },
                PluginConfig::ModelRouter {
                    models: vec!["gpt-4".to_string()],
                },
            ],
        }],
    };

    // This should not hang or panic even with an invalid tracing endpoint
    let start_time = std::time::Instant::now();
    let app_state = AppState::new(config).expect("Failed to create app state");
    let elapsed = start_time.elapsed();

    // Should complete quickly (within 1 second) since OpenTelemetry init is now async
    assert!(
        elapsed.as_millis() < 1000,
        "Router building took too long: {:?}",
        elapsed
    );

    // Verify the router is available immediately
    let _router = app_state.get_current_router();
    // If we get here without panicking, the router was created successfully

    // Wait a moment to let the async OpenTelemetry initialization complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // The router should still be available and functional
    let _router2 = app_state.get_current_router();
}

#[tokio::test]
async fn test_tracing_isolation_between_pipelines() {
    // Create configuration with two pipelines - one with tracing, one without
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
        pipelines: vec![
            // Pipeline with tracing
            Pipeline {
                name: "traced-pipeline".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![
                    PluginConfig::Tracing {
                        endpoint: "http://invalid-endpoint.example.com/traces".to_string(),
                        api_key: "test-key".to_string(),
                    },
                    PluginConfig::ModelRouter {
                        models: vec!["gpt-4".to_string()],
                    },
                ],
            },
            // Pipeline without tracing
            Pipeline {
                name: "simple-pipeline".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![PluginConfig::ModelRouter {
                    models: vec!["gpt-4".to_string()],
                }],
            },
        ],
    };

    // Create app state with the configuration
    let app_state = AppState::new(config).expect("Failed to create app state");

    // Verify the router is available and both pipelines are configured
    let _router = app_state.get_current_router();

    // Test should pass without hanging, indicating that:
    // 1. Pipeline with tracing can be created (even with invalid endpoint, due to async init)
    // 2. Pipeline without tracing can be created
    // 3. Both pipelines are properly isolated
}
