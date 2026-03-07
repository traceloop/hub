use hub_lib::config::hash::{calculate_config_hash, configs_are_equal};
use hub_lib::types::{GatewayConfig, Provider, ProviderType};
use std::collections::HashMap;

#[test]
fn test_identical_configs_have_same_hash() {
    let config1 = GatewayConfig {
        general: None,
        guardrails: None,
        providers: vec![Provider {
            key: "test".to_string(),
            r#type: ProviderType::OpenAI,
            api_key: "key".to_string(),
            params: Default::default(),
        }],
        models: vec![],
        pipelines: vec![],
    };

    let config2 = config1.clone();

    assert!(configs_are_equal(&config1, &config2));
    assert_eq!(
        calculate_config_hash(&config1),
        calculate_config_hash(&config2)
    );
}

#[test]
fn test_different_configs_have_different_hashes() {
    let config1 = GatewayConfig {
        general: None,
        guardrails: None,
        providers: vec![Provider {
            key: "test1".to_string(),
            r#type: ProviderType::OpenAI,
            api_key: "key".to_string(),
            params: Default::default(),
        }],
        models: vec![],
        pipelines: vec![],
    };

    let config2 = GatewayConfig {
        general: None,
        guardrails: None,
        providers: vec![Provider {
            key: "test2".to_string(), // Different key
            r#type: ProviderType::OpenAI,
            api_key: "key".to_string(),
            params: Default::default(),
        }],
        models: vec![],
        pipelines: vec![],
    };

    assert!(!configs_are_equal(&config1, &config2));
    assert_ne!(
        calculate_config_hash(&config1),
        calculate_config_hash(&config2)
    );
}

#[test]
fn test_params_order_independence() {
    let mut params1 = HashMap::new();
    params1.insert("a".to_string(), "1".to_string());
    params1.insert("b".to_string(), "2".to_string());

    let mut params2 = HashMap::new();
    params2.insert("b".to_string(), "2".to_string());
    params2.insert("a".to_string(), "1".to_string());

    let config1 = GatewayConfig {
        general: None,
        guardrails: None,
        providers: vec![Provider {
            key: "test".to_string(),
            r#type: ProviderType::OpenAI,
            api_key: "key".to_string(),
            params: params1,
        }],
        models: vec![],
        pipelines: vec![],
    };

    let config2 = GatewayConfig {
        general: None,
        guardrails: None,
        providers: vec![Provider {
            key: "test".to_string(),
            r#type: ProviderType::OpenAI,
            api_key: "key".to_string(),
            params: params2,
        }],
        models: vec![],
        pipelines: vec![],
    };

    // Should be equal despite different insertion order
    assert!(configs_are_equal(&config1, &config2));
}
