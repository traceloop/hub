use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use hub_gateway_core_types::GatewayConfig;

/// Calculate a hash for the configuration to detect changes
pub fn calculate_config_hash(config: &GatewayConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    config.hash(&mut hasher);
    hasher.finish()
}

/// Check if two configurations are equal by comparing their hashes
/// This is more efficient than deep comparison for large configs
pub fn configs_are_equal(config1: &GatewayConfig, config2: &GatewayConfig) -> bool {
    calculate_config_hash(config1) == calculate_config_hash(config2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hub_gateway_core_types::Provider;

    #[test]
    fn test_identical_configs_have_same_hash() {
        let config1 = GatewayConfig {
            general: None,
            providers: vec![Provider {
                key: "test".to_string(),
                r#type: "openai".to_string(),
                api_key: "key".to_string(),
                params: Default::default(),
            }],
            models: vec![],
            pipelines: vec![],
        };

        let config2 = config1.clone();
        
        assert!(configs_are_equal(&config1, &config2));
        assert_eq!(calculate_config_hash(&config1), calculate_config_hash(&config2));
    }

    #[test]
    fn test_different_configs_have_different_hashes() {
        let config1 = GatewayConfig {
            general: None,
            providers: vec![Provider {
                key: "test1".to_string(),
                r#type: "openai".to_string(),
                api_key: "key".to_string(),
                params: Default::default(),
            }],
            models: vec![],
            pipelines: vec![],
        };

        let config2 = GatewayConfig {
            general: None,
            providers: vec![Provider {
                key: "test2".to_string(), // Different key
                r#type: "openai".to_string(),
                api_key: "key".to_string(),
                params: Default::default(),
            }],
            models: vec![],
            pipelines: vec![],
        };

        assert!(!configs_are_equal(&config1, &config2));
        assert_ne!(calculate_config_hash(&config1), calculate_config_hash(&config2));
    }

    #[test]
    fn test_params_order_independence() {
        use std::collections::HashMap;

        let mut params1 = HashMap::new();
        params1.insert("a".to_string(), "1".to_string());
        params1.insert("b".to_string(), "2".to_string());

        let mut params2 = HashMap::new();
        params2.insert("b".to_string(), "2".to_string());
        params2.insert("a".to_string(), "1".to_string());

        let config1 = GatewayConfig {
            general: None,
            providers: vec![Provider {
                key: "test".to_string(),
                r#type: "openai".to_string(),
                api_key: "key".to_string(),
                params: params1,
            }],
            models: vec![],
            pipelines: vec![],
        };

        let config2 = GatewayConfig {
            general: None,
            providers: vec![Provider {
                key: "test".to_string(),
                r#type: "openai".to_string(),
                api_key: "key".to_string(),
                params: params2,
            }],
            models: vec![],
            pipelines: vec![],
        };

        // Should be equal despite different insertion order
        assert!(configs_are_equal(&config1, &config2));
    }
} 