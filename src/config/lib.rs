// pub mod validation;

use hub_gateway_core_types::{GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider};
use serde::Deserialize;
// std::collections::HashMap is used by serde_yaml for flatten, but not directly here otherwise.

// Intermediate struct for deserializing pipelines from YAML
#[derive(Deserialize, Debug)]
struct YamlCompatiblePipeline {
    name: String,
    r#type: PipelineType,
    #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    plugins: Vec<PluginConfig>,
    #[serde(default = "default_enabled_true_lib")]
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
        pipelines: yaml_root.pipelines.into_iter().map(|p_yaml| {
            // Map to core_types::Pipeline. ee_id and enabled are no longer fields here.
            Pipeline {
                name: p_yaml.name,
                r#type: p_yaml.r#type,
                plugins: p_yaml.plugins,
                // p_yaml.enabled is parsed from YAML but not stored in core Pipeline struct
            }
        }).collect(),
    };

    Ok(gateway_config)
}
