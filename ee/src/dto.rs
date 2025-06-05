use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use utoipa::ToSchema;

/// Enum representing the type of LLM provider.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "bedrock")]
    Bedrock,
    // Add other types like Gemini, Anthropic here as they are supported
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Azure => write!(f, "azure"),
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::Bedrock => write!(f, "bedrock"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = String; // Or a custom error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "azure" => Ok(ProviderType::Azure),
            "openai" => Ok(ProviderType::OpenAI),
            "bedrock" => Ok(ProviderType::Bedrock),
            _ => Err(format!("Unknown provider type: {}", s)),
        }
    }
}

/// Configuration specific to OpenAI providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct OpenAIProviderConfig {
    pub api_key: String,
    pub organization_id: Option<String>,
}

/// Configuration specific to Azure OpenAI providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct AzureProviderConfig {
    pub api_key: String,
    pub resource_name: String,
    pub api_version: String,
}

/// Configuration specific to AWS Bedrock providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct BedrockProviderConfig {
    pub aws_access_key_id: Option<String>,
    pub aws_secret_access_key: Option<String>,
    pub aws_session_token: Option<String>,
    pub region: String,
}

/// Enum to hold the configuration for different provider types.
/// The correct variant will be determined by the provider_type field in the request.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq)]
#[serde(untagged)]
pub enum ProviderConfig {
    Azure(AzureProviderConfig),
    OpenAI(OpenAIProviderConfig),
    Bedrock(BedrockProviderConfig),
}

// --- API Request DTOs ---

/// Request payload for creating a new provider configuration.
#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct CreateProviderRequest {
    /// A unique, user-friendly name for this provider configuration.
    pub name: String,
    /// The type of the LLM provider.
    #[schema(value_type = String)] // Helps Utoipa represent the enum as a string
    pub provider_type: ProviderType,
    /// The specific configuration details for the provider type.
    pub config: ProviderConfig,
    /// Whether this provider configuration is enabled. Defaults to true if not provided.
    pub enabled: Option<bool>,
}

/// Request payload for updating an existing provider configuration.
#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct UpdateProviderRequest {
    /// A new unique, user-friendly name for this provider configuration.
    pub name: Option<String>,
    /// The new specific configuration details. The 'type' within this config
    /// must match the existing provider's type.
    pub config: Option<ProviderConfig>,
    /// Whether this provider configuration should be enabled.
    pub enabled: Option<bool>,
}

// --- API Response DTO ---

/// Response payload representing a provider configuration.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct ProviderResponse {
    pub id: Uuid,
    pub name: String,
    #[schema(value_type = String)]
    pub provider_type: ProviderType,
    pub config: ProviderConfig,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// --- Model Definition DTOs ---

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, ToSchema)]
pub struct CreateModelDefinitionRequest {
    #[schema(example = "gpt-4o-openai")]
    pub key: String,
    #[schema(example = "gpt-4o")]
    pub model_type: String,
    pub provider_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = json!({"deployment": "my-deployment-id"}))]
    pub config_details: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, ToSchema)]
pub struct UpdateModelDefinitionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "gpt-4o-openai-updated")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = "gpt-4o-mini")]
    pub model_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")] // Option<Value> allows sending null to clear
    #[schema(example = json!({}))]
    pub config_details: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, ToSchema)]
pub struct ModelDefinitionResponse {
    pub id: Uuid,
    pub key: String,
    pub model_type: String,
    // It's often useful to return the full provider details, or at least key info.
    // If ProviderResponse is too heavy, consider a slimmer ProviderInfo struct.
    pub provider: ProviderResponse,
    pub config_details: serde_json::Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// --- Pipeline & Model Routing DTOs ---

/// Represents a single model entry within a model router's configuration.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct ModelRouterModelEntryDto {
    /// The key of the ModelDefinition to use.
    #[schema(example = "gpt-4o-openai")]
    pub key: String,
    /// Priority of this model in the routing strategy (lower is higher priority).
    #[schema(example = 0)]
    pub priority: i32,
    // Future fields like 'weight', 'max_tokens_override', etc. can be added here.
    // pub weight: Option<f32>,
    // pub config_override: Option<serde_json::Value>,
}

/// Defines the strategy for how models are selected by the model router.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ModelRouterStrategyDto {
    #[default]
    Simple,
    /// Tries models in order of priority until one succeeds.
    OrderedFallback,
    /// Future: Randomly selects a model based on weights.
    WeightedRandom, // Add other strategies as needed
}


/// Configuration specific to the 'model-router' plugin.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct ModelRouterConfigDto {
    #[schema(value_type = String, example = "ordered_fallback")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<ModelRouterStrategyDto>,
    pub models: Vec<ModelRouterModelEntryDto>,
}

/// Supported plugin types for pipelines.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PluginType {
    /// Model routing plugin for selecting models based on strategy.
    ModelRouter,
    /// Logging plugin for request/response logging.
    Logging,
    /// Tracing plugin for distributed tracing.
    Tracing,
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginType::ModelRouter => write!(f, "model-router"),
            PluginType::Logging => write!(f, "logging"),
            PluginType::Tracing => write!(f, "tracing"),
        }
    }
}

impl std::str::FromStr for PluginType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "model-router" => Ok(PluginType::ModelRouter),
            "logging" => Ok(PluginType::Logging),
            "tracing" => Ok(PluginType::Tracing),
            _ => Err(format!("Unknown plugin type: {}", s)),
        }
    }
}

/// Represents a generic plugin configuration for a pipeline.
/// The `config_data` field will be interpreted based on `plugin_type`.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct PipelinePluginConfigDto {
    /// Type of the plugin.
    #[schema(value_type = String, example = "model-router")]
    pub plugin_type: PluginType,
    /// JSON object containing the specific configuration for this plugin.
    /// For "model-router", this should deserialize to ModelRouterConfigDto.
    #[schema(example = json!({"strategy": "ordered_fallback", "models": [{"key": "gpt-4o", "priority": 0}]}))]
    pub config_data: serde_json::Value,
    /// Whether this plugin is enabled within the pipeline.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Order of execution for this plugin within the pipeline (lower is earlier).
    #[serde(default)]
    pub order_in_pipeline: i32,
}

fn default_true() -> bool {
    true
}

/// Request payload for creating a new pipeline.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct CreatePipelineRequestDto {
    /// A unique, user-friendly name for this pipeline.
    #[schema(example = "default_chat_pipeline")]
    pub name: String,
    /// Type of the pipeline (e.g., "chat", "completion").
    #[schema(example = "chat")]
    pub pipeline_type: String,
    /// Optional description for the pipeline.
    pub description: Option<String>,
    /// List of plugin configurations for this pipeline.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PipelinePluginConfigDto>,
    /// Whether this pipeline is enabled. Defaults to true.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Request payload for updating an existing pipeline.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct UpdatePipelineRequestDto {
    /// A new unique, user-friendly name for this pipeline.
    #[schema(example = "default_chat_pipeline_v2")]
    pub name: Option<String>,
    /// New type of the pipeline.
    #[schema(example = "chat_experimental")]
    pub pipeline_type: Option<String>,
    /// New optional description for the pipeline.
    pub description: Option<String>,
    /// New list of plugin configurations. If provided, this will replace all existing plugins.
    /// To modify a single plugin, fetch the pipeline, modify the plugins list, and send it back.
    pub plugins: Option<Vec<PipelinePluginConfigDto>>,
    /// Whether this pipeline should be enabled.
    pub enabled: Option<bool>,
}

/// Response payload representing a pipeline.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct PipelineResponseDto {
    pub id: Uuid,
    pub name: String,
    pub pipeline_type: String,
    pub description: Option<String>,
    pub plugins: Vec<PipelinePluginConfigDto>, // For simplicity, returning the same DTO used in create/update. Could be a different one if needed.
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
