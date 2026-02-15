use serde::{Deserialize, Serialize};
// use serde_json::Value as JsonValue; // Removed
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use utoipa::ToSchema;

fn default_trace_content_enabled() -> bool {
    true
}

fn no_api_key() -> String {
    "".to_string()
}

fn default_log_level_core() -> String {
    "warning".to_string()
}

/// Enum representing the type of LLM provider.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "bedrock")]
    Bedrock,
    #[serde(rename = "vertexai")]
    VertexAI,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Azure => write!(f, "azure"),
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::Bedrock => write!(f, "bedrock"),
            ProviderType::VertexAI => write!(f, "vertexai"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = String; // Or a custom error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "azure" => Ok(ProviderType::Azure),
            "openai" => Ok(ProviderType::OpenAI),
            "anthropic" => Ok(ProviderType::Anthropic),
            "bedrock" => Ok(ProviderType::Bedrock),
            "vertexai" => Ok(ProviderType::VertexAI),
            _ => Err(format!("Unknown provider type: {s}")),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Provider {
    pub key: String,
    pub r#type: ProviderType,

    #[serde(default = "no_api_key")]
    pub api_key: String,

    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrails: Option<crate::guardrails::types::GuardrailsConfig>,
}
