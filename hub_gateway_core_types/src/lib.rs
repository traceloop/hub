use serde::{Deserialize, Serialize};
// use serde_json::Value as JsonValue; // Removed
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
// Uuid is no longer needed here if ee_id is removed
// use uuid::Uuid;

// Helper for defaulting boolean to true for serde
// fn bool_true() -> bool { // Removed
//     true
// }

fn default_trace_content_enabled() -> bool {
    true
}

fn no_api_key() -> String {
    "".to_string()
}

fn default_log_level_core() -> String {
    "warning".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Provider {
    pub key: String,
    pub r#type: String, // e.g., "openai", "azure"

    #[serde(default = "no_api_key")]
    pub api_key: String,

    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
    // ee_id: Option<Uuid>, // Removed
    // enabled: bool, // Removed
}

impl Hash for Provider {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.r#type.hash(state);
        self.api_key.hash(state);
        // Hash the params by sorting keys and hashing key-value pairs
        let mut params_vec: Vec<_> = self.params.iter().collect();
        params_vec.sort_by_key(|(k, _)| *k);
        for (k, v) in params_vec {
            k.hash(state);
            v.hash(state);
        }
    }
}

// Renamed from SharedModelConfig
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelConfig {
    pub key: String,
    pub r#type: String,   // Actual model name, e.g., "gpt-4o"
    pub provider: String, // Key of the Provider struct

    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
    // ee_id: Option<Uuid>, // Removed
    // enabled: bool, // Removed
}

impl Hash for ModelConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.r#type.hash(state);
        self.provider.hash(state);
        // Hash the params by sorting keys and hashing key-value pairs
        let mut params_vec: Vec<_> = self.params.iter().collect();
        params_vec.sort_by_key(|(k, _)| *k);
        for (k, v) in params_vec {
            k.hash(state);
            v.hash(state);
        }
    }
}

// Renamed from SharedPipelineType (name is identical to original in src)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PipelineType {
    Chat,
    Completion,
    Embeddings,
}

// Renamed from SharedPipelinePluginConfig (name is identical to original in src)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PluginConfig {
    Logging {
        #[serde(default = "default_log_level_core")] // Renamed default fn to avoid conflict
        level: String,
    },
    Tracing {
        endpoint: String,
        api_key: String,
    },
    ModelRouter {
        models: Vec<String>,
    },
}

// Renamed from SharedPipelineConfig
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Hash)]
pub struct Pipeline {
    pub name: String,
    pub r#type: PipelineType,

    // #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginConfig>,
    // ee_id: Option<Uuid>, // Removed
    // enabled: bool, // Removed
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Hash)]
pub struct General {
    #[serde(default = "default_trace_content_enabled")]
    pub trace_content_enabled: bool,
}

// GatewayConfig name remains the same
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Hash)]
pub struct GatewayConfig {
    pub general: Option<General>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<Provider>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<ModelConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipelines: Vec<Pipeline>,
}

// Example of how ee DTOs could be transformed (conceptual, to be implemented in ee crate)
/*
impl From<ee_crate_dto::ProviderResponse> for SharedProviderConfig {
    fn from(dto: ee_crate_dto::ProviderResponse) -> Self {
        // ... transformation logic ...
    }
}
*/
