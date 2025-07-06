use crate::types::{GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider};
use serde::Deserialize;
use std::sync::OnceLock;
// std::collections::HashMap is used by serde_yaml for flatten, but not directly here otherwise.

pub static TRACE_CONTENT_ENABLED: OnceLock<bool> = OnceLock::new();
// Intermediate struct for deserializing pipelines from YAML
#[derive(Deserialize, Debug)]
struct YamlCompatiblePipeline {
    name: String,
    r#type: PipelineType,
    #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    plugins: Vec<PluginConfig>,
    #[serde(default = "default_enabled_true_lib")]
    #[allow(dead_code)]
    enabled: bool, // Keep for YAML parsing, but won't be mapped to core Pipeline
}

fn default_enabled_true_lib() -> bool {
    true
}

// Intermediate struct for the top-level YAML structure if needed,
// especially if other parts of GatewayConfig also had complex YAML-specific attributes.
// For now, assuming only pipelines might need this special handling for `singleton_map_recursive`.
#[derive(Deserialize, Debug)]
struct YamlRoot {
    #[serde(default)]
    providers: Vec<Provider>,
    #[serde(default)]
    models: Vec<ModelConfig>,
    #[serde(default)]
    pipelines: Vec<YamlCompatiblePipeline>,
}

pub fn load_config(path: &str) -> Result<GatewayConfig, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let yaml_root: YamlRoot = serde_yaml::from_str(&contents)?;

    let gateway_config = GatewayConfig {
        providers: yaml_root.providers,
        models: yaml_root.models,
        pipelines: yaml_root
            .pipelines
            .into_iter()
            .map(|p_yaml| {
                // Map to core_types::Pipeline. ee_id and enabled are no longer fields here.
                Pipeline {
                    name: p_yaml.name,
                    r#type: p_yaml.r#type,
                    plugins: p_yaml.plugins,
                    // p_yaml.enabled is parsed from YAML but not stored in core Pipeline struct
                }
            })
            .collect(),
        general: None,
    };
    let _ = TRACE_CONTENT_ENABLED.set(
        gateway_config
            .general
            .as_ref()
            .is_none_or(|g| g.trace_content_enabled),
    );

    Ok(gateway_config)
}

fn parse_env_var_bool(var: &str) -> Option<bool> {
    match var.to_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn get_trace_content_enabled() -> bool {
    // Always check environment variable first (useful for database mode)
    if let Ok(env_value) = std::env::var("TRACE_CONTENT_ENABLED") {
        if let Some(val) = parse_env_var_bool(&env_value) {
            return val;
        }
    }
    // Fall back to config value or default true
    *TRACE_CONTENT_ENABLED.get_or_init(|| true)
}
