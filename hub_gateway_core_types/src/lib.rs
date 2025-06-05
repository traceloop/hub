use serde::{Deserialize, Serialize};
// use serde_json::Value as JsonValue; // Removed
use std::collections::HashMap;
// Uuid is no longer needed here if ee_id is removed
// use uuid::Uuid; 

// Helper for defaulting boolean to true for serde
// fn bool_true() -> bool { // Removed
//     true
// }

// Helper for defaulting api_key to empty string
fn default_empty_string() -> String {
    "".to_string()
}

// Renamed from SharedProviderConfig
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Provider {
    pub key: String,
    pub r#type: String, // e.g., "openai", "azure"
    
    #[serde(default = "default_empty_string")]
    pub api_key: String,
    
    #[serde(flatten,default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
    
    // ee_id: Option<Uuid>, // Removed
    // enabled: bool, // Removed
}

// Renamed from SharedModelConfig
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelConfig {
    pub key: String,
    pub r#type: String, // Actual model name, e.g., "gpt-4o"
    pub provider: String, // Key of the Provider struct
    
    #[serde(flatten,default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
    
    // ee_id: Option<Uuid>, // Removed
    // enabled: bool, // Removed
}

// Renamed from SharedPipelineType (name is identical to original in src)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PipelineType {
    Chat,
    Completion,
    Embeddings,
}

// Renamed from SharedPipelinePluginConfig (name is identical to original in src)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")] 
pub enum PluginConfig {
    Logging {
        #[serde(default = "default_log_level_core")] // Renamed default fn to avoid conflict
        level: String,
    },
    Tracing {
        endpoint: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
    },
    ModelRouter {
        models: Vec<String>,
    },
}

fn default_log_level_core() -> String { // Renamed default fn
    "warning".to_string()
}

// Renamed from SharedPipelineConfig
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub name: String,
    pub r#type: PipelineType,
    
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginConfig>,
    
    // ee_id: Option<Uuid>, // Removed
    // enabled: bool, // Removed
}

// GatewayConfig name remains the same
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct GatewayConfig {
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