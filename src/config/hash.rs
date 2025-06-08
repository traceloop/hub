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
 