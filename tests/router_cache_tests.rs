use hub_gateway_core_types::{
    GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider,
};
use hub_lib::state::AppState;
use std::sync::Arc;

#[tokio::test]
async fn test_router_cache_basic_functionality() {
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

    // Initially, no router should be cached
    assert!(app_state.get_cached_pipeline_router().is_none());

    // Trigger router cache by calling the cache management
    app_state.with_router_cache(|cache| {
        assert!(cache.get().is_none());
    });

    // Test cache invalidation
    app_state.invalidate_cached_router();
    assert!(app_state.get_cached_pipeline_router().is_none());
}

#[tokio::test]
async fn test_router_cache_invalidation() {
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

    // Test that cache invalidation works
    app_state.invalidate_cached_router();
    assert!(app_state.get_cached_pipeline_router().is_none());

    // Test that we can rebuild the router
    let rebuild_result = app_state.rebuild_pipeline_router_now();
    assert!(rebuild_result.is_ok());

    // After rebuilding, router should be cached
    assert!(app_state.get_cached_pipeline_router().is_some());

    // Test invalidation again
    app_state.invalidate_cached_router();
    assert!(app_state.get_cached_pipeline_router().is_none());
}

#[tokio::test]
async fn test_configuration_update_invalidates_cache() {
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

    let app_state = Arc::new(AppState::new(initial_config).expect("Failed to create app state"));

    // Build and cache a router
    let rebuild_result = app_state.rebuild_pipeline_router_now();
    assert!(rebuild_result.is_ok());
    assert!(app_state.get_cached_pipeline_router().is_some());

    // Create a new configuration with additional pipeline
    let updated_config = GatewayConfig {
        general: None,
        providers: vec![Provider {
            key: "test-provider".to_string(),
            r#type: "openai".to_string(),
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
                name: "new-pipeline".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![PluginConfig::ModelRouter {
                    models: vec!["gpt-3.5-turbo".to_string()],
                }],
            },
        ],
    };

    // Update configuration - this should invalidate the cache
    let update_result = app_state.try_update_config_and_registries(updated_config);
    assert!(update_result.is_ok());

    // Verify that the configuration was updated
    let snapshot = app_state.config_snapshot();
    assert_eq!(snapshot.config.pipelines.len(), 2);
    assert_eq!(snapshot.config.models.len(), 2);

    // The cache should have been rebuilt automatically
    assert!(app_state.get_cached_pipeline_router().is_some());
}

#[tokio::test]
async fn test_concurrent_cache_access() {
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

    // Spawn multiple tasks that access the cache concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let app_state_clone = app_state.clone();
        let handle = tokio::spawn(async move {
            // Some tasks invalidate cache, others try to access it
            if i % 2 == 0 {
                app_state_clone.invalidate_cached_router();
            } else {
                let _ = app_state_clone.get_cached_pipeline_router();
            }

            // All tasks try to rebuild
            let _ = app_state_clone.rebuild_pipeline_router_now();
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // After all concurrent operations, we should have a cached router
    assert!(app_state.get_cached_pipeline_router().is_some());
}
