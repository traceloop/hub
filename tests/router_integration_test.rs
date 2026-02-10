use hub_lib::types::{
    GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider, ProviderType,
};
use hub_lib::{config, state::AppState};
use std::sync::Arc;

#[tokio::test]
async fn test_router_integration_flow() {
    // Test 1: Empty configuration
    let empty_config = GatewayConfig {
        general: None,
        providers: vec![],
        models: vec![],
        pipelines: vec![],
        guardrails: None,
    };

    let app_state = Arc::new(AppState::new(empty_config).expect("Failed to create app state"));

    // With empty config, router should still be available (fallback router)
    let _router = app_state.get_current_router();

    // Test 2: Valid configuration
    let valid_config = GatewayConfig {
        general: None,
        guardrails: None,
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

    // Test 3: Configuration update
    let update_result = app_state.update_config(valid_config);
    assert!(update_result.is_ok(), "Configuration update should succeed");

    // Verify configuration was updated
    let snapshot = app_state.config_snapshot();
    assert_eq!(snapshot.config.providers.len(), 1);
    assert_eq!(snapshot.config.models.len(), 1);
    assert_eq!(snapshot.config.pipelines.len(), 1);
    assert_eq!(snapshot.config.providers[0].key, "test-provider");
    assert_eq!(snapshot.config.models[0].key, "gpt-4");
    assert_eq!(snapshot.config.pipelines[0].name, "default");

    // Test 4: Router is always available with simplified approach
    let _current_router = app_state.get_current_router();
    // Router should always be available

    // Test 6: Invalid configuration rejection
    let invalid_config = GatewayConfig {
        general: None,
        guardrails: None,
        providers: vec![Provider {
            key: "test-provider".to_string(),
            r#type: ProviderType::OpenAI,
            api_key: "test-key".to_string(),
            params: Default::default(),
        }],
        models: vec![ModelConfig {
            key: "gpt-4".to_string(),
            r#type: "gpt-4".to_string(),
            provider: "nonexistent-provider".to_string(), // Invalid provider reference
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

    // Validation should fail
    let validation_result = config::validation::validate_gateway_config(&invalid_config);
    assert!(validation_result.is_err());

    let errors = validation_result.unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors[0].contains("references non-existent provider"));

    // Test 7: Multiple pipeline configuration
    let multi_pipeline_config = GatewayConfig {
        general: None,
        guardrails: None,
        providers: vec![Provider {
            key: "test-provider".to_string(),
            r#type: ProviderType::OpenAI,
            api_key: "test-key".to_string(),
            params: Default::default(),
        }],
        models: vec![
            ModelConfig {
                key: "gpt-4".to_string(),
                r#type: "gpt-4".to_string(),
                provider: "test-provider".to_string(),
                params: Default::default(),
            },
            ModelConfig {
                key: "gpt-3.5-turbo".to_string(),
                r#type: "gpt-3.5-turbo".to_string(),
                provider: "test-provider".to_string(),
                params: Default::default(),
            },
        ],
        pipelines: vec![
            Pipeline {
                name: "default".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![PluginConfig::ModelRouter {
                    models: vec!["gpt-4".to_string()],
                }],
            },
            Pipeline {
                name: "fast".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![PluginConfig::ModelRouter {
                    models: vec!["gpt-3.5-turbo".to_string()],
                }],
            },
        ],
    };

    let multi_update_result = app_state.update_config(multi_pipeline_config);
    assert!(
        multi_update_result.is_ok(),
        "Multi-pipeline configuration update should succeed"
    );

    // Verify multi-pipeline configuration
    let multi_snapshot = app_state.config_snapshot();
    assert_eq!(multi_snapshot.config.pipelines.len(), 2);
    assert_eq!(multi_snapshot.config.models.len(), 2);

    let pipeline_names: Vec<&String> = multi_snapshot
        .config
        .pipelines
        .iter()
        .map(|p| &p.name)
        .collect();
    assert!(pipeline_names.contains(&&"default".to_string()));
    assert!(pipeline_names.contains(&&"fast".to_string()));
}

#[tokio::test]
async fn test_concurrent_configuration_updates() {
    let initial_config = GatewayConfig {
        general: None,
        guardrails: None,
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

    let app_state = Arc::new(AppState::new(initial_config).expect("Failed to create app state"));

    // Spawn multiple concurrent tasks that update configuration and access cache
    let mut handles = vec![];
    for i in 0..10 {
        let app_state_clone = app_state.clone();
        let handle = tokio::spawn(async move {
            // Create a slightly different configuration for each task
            let config = GatewayConfig {
                general: None,
                guardrails: None,
                providers: vec![Provider {
                    key: format!("provider-{}", i),
                    r#type: ProviderType::OpenAI,
                    api_key: "test-key".to_string(),
                    params: Default::default(),
                }],
                models: vec![ModelConfig {
                    key: format!("model-{}", i),
                    r#type: "gpt-4".to_string(),
                    provider: format!("provider-{}", i),
                    params: Default::default(),
                }],
                pipelines: vec![Pipeline {
                    name: format!("pipeline-{}", i),
                    r#type: PipelineType::Chat,
                    plugins: vec![PluginConfig::ModelRouter {
                        models: vec![format!("model-{}", i)],
                    }],
                }],
            };

            // Some tasks update configuration, others access router
            if i % 2 == 0 {
                let _ = app_state_clone.update_config(config);
            } else {
                let _ = app_state_clone.get_current_router();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // After all concurrent operations, the app state should still be functional
    let final_snapshot = app_state.config_snapshot();
    assert!(!final_snapshot.config.providers.is_empty());
    assert!(!final_snapshot.config.models.is_empty());
    assert!(!final_snapshot.config.pipelines.is_empty());
}
