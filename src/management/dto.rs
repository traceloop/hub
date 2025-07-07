use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use utoipa::ToSchema;

/// Represents different ways to store and retrieve secrets
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum SecretObject {
    #[serde(rename = "literal")]
    Literal {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        encrypted: Option<bool>, // Future: indicates if value is encrypted
    },

    #[serde(rename = "kubernetes")]
    Kubernetes {
        secret_name: String,
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        namespace: Option<String>,
    },

    #[serde(rename = "environment")]
    Environment { variable_name: String },
}

impl SecretObject {
    /// Create a literal secret object from a string value
    pub fn literal(value: String) -> Self {
        Self::Literal {
            value,
            encrypted: None,
        }
    }

    /// Create a Kubernetes secret reference
    pub fn kubernetes(secret_name: String, key: String, namespace: Option<String>) -> Self {
        Self::Kubernetes {
            secret_name,
            key,
            namespace,
        }
    }

    /// Create an environment variable reference
    pub fn environment(variable_name: String) -> Self {
        Self::Environment { variable_name }
    }
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

/// Configuration specific to OpenAI providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct OpenAIProviderConfig {
    pub api_key: SecretObject,
    pub organization_id: Option<String>,
}

/// Configuration specific to Anthropic providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct AnthropicProviderConfig {
    pub api_key: SecretObject,
}

/// Configuration specific to Azure OpenAI providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct AzureProviderConfig {
    pub api_key: SecretObject,
    pub resource_name: String,
    pub api_version: String,
    pub base_url: Option<String>,
}

/// Configuration specific to AWS Bedrock providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct BedrockProviderConfig {
    pub aws_access_key_id: Option<SecretObject>,
    pub aws_secret_access_key: Option<SecretObject>,
    pub aws_session_token: Option<SecretObject>,
    pub region: String,
    pub use_iam_role: Option<bool>,
    pub inference_profile_id: Option<String>,
}

/// Configuration specific to Google VertexAI providers.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq, Eq)]
pub struct VertexAIProviderConfig {
    pub project_id: String,
    pub location: String,
    pub credentials_path: Option<String>,
    pub api_key: Option<SecretObject>,
}

/// Enum to hold the configuration for different provider types.
/// The correct variant will be determined by the provider_type field in the request.
#[derive(Serialize, Deserialize, Debug, ToSchema, Clone, PartialEq)]
#[serde(untagged)]
pub enum ProviderConfig {
    VertexAI(VertexAIProviderConfig),   // 4 fields - most specific
    Azure(AzureProviderConfig),         // 3 fields
    Bedrock(BedrockProviderConfig),     // 4 fields but some optional
    OpenAI(OpenAIProviderConfig),       // 2 fields (1 optional) - must come before Anthropic
    Anthropic(AnthropicProviderConfig), // 1 field - least specific, must be last
}

// --- API Request DTOs ---

/// Request payload for creating a new provider configuration.
#[derive(Serialize, Debug, ToSchema)]
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

impl<'de> serde::Deserialize<'de> for CreateProviderRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(serde::Deserialize)]
        struct CreateProviderRequestHelper {
            name: String,
            provider_type: ProviderType,
            config: serde_json::Value,
            enabled: Option<bool>,
        }

        let helper = CreateProviderRequestHelper::deserialize(deserializer)?;

        let config = match helper.provider_type {
            ProviderType::OpenAI => {
                let config: OpenAIProviderConfig =
                    serde_json::from_value(helper.config).map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize OpenAI config: {e}"))
                    })?;
                ProviderConfig::OpenAI(config)
            }
            ProviderType::Azure => {
                let config: AzureProviderConfig =
                    serde_json::from_value(helper.config).map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize Azure config: {e}"))
                    })?;
                ProviderConfig::Azure(config)
            }
            ProviderType::Anthropic => {
                let config: AnthropicProviderConfig = serde_json::from_value(helper.config)
                    .map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize Anthropic config: {e}"))
                    })?;
                ProviderConfig::Anthropic(config)
            }
            ProviderType::Bedrock => {
                let config: BedrockProviderConfig =
                    serde_json::from_value(helper.config).map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize Bedrock config: {e}"))
                    })?;
                ProviderConfig::Bedrock(config)
            }
            ProviderType::VertexAI => {
                let config: VertexAIProviderConfig = serde_json::from_value(helper.config)
                    .map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize VertexAI config: {e}"))
                    })?;
                ProviderConfig::VertexAI(config)
            }
        };

        Ok(CreateProviderRequest {
            name: helper.name,
            provider_type: helper.provider_type,
            config,
            enabled: helper.enabled,
        })
    }
}

/// Request payload for updating an existing provider configuration.
#[derive(Serialize, Debug, ToSchema)]
pub struct UpdateProviderRequest {
    /// A new unique, user-friendly name for this provider configuration.
    pub name: Option<String>,
    /// The new specific configuration details. The 'type' within this config
    /// must match the existing provider's type.
    pub config: Option<ProviderConfig>,
    /// Whether this provider configuration should be enabled.
    pub enabled: Option<bool>,
}

impl<'de> serde::Deserialize<'de> for UpdateProviderRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(serde::Deserialize)]
        struct UpdateProviderRequestHelper {
            name: Option<String>,
            config: Option<serde_json::Value>,
            enabled: Option<bool>,
        }

        let helper = UpdateProviderRequestHelper::deserialize(deserializer)?;

        // For update requests, we can't determine the provider type from the request alone
        // So we'll just store the config as a raw value and let the service layer handle it
        let config = helper
            .config
            .map(|config_value| {
                // We'll try to deserialize as untagged enum, but this might still have ambiguity
                // The service layer should handle this by using the existing provider's type
                serde_json::from_value(config_value)
                    .map_err(|e| D::Error::custom(format!("Failed to deserialize config: {e}")))
            })
            .transpose()?;

        Ok(UpdateProviderRequest {
            name: helper.name,
            config,
            enabled: helper.enabled,
        })
    }
}

// --- API Response DTO ---

/// Response payload representing a provider configuration.
#[derive(Debug, Serialize, ToSchema, PartialEq, Clone)]
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

impl<'de> serde::Deserialize<'de> for ProviderResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(serde::Deserialize)]
        struct ProviderResponseHelper {
            id: Uuid,
            name: String,
            provider_type: ProviderType,
            config: serde_json::Value,
            enabled: bool,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
        }

        let helper = ProviderResponseHelper::deserialize(deserializer)?;

        let config = match helper.provider_type {
            ProviderType::OpenAI => {
                let config: OpenAIProviderConfig =
                    serde_json::from_value(helper.config).map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize OpenAI config: {e}"))
                    })?;
                ProviderConfig::OpenAI(config)
            }
            ProviderType::Azure => {
                let config: AzureProviderConfig =
                    serde_json::from_value(helper.config).map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize Azure config: {e}"))
                    })?;
                ProviderConfig::Azure(config)
            }
            ProviderType::Anthropic => {
                let config: AnthropicProviderConfig = serde_json::from_value(helper.config)
                    .map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize Anthropic config: {e}"))
                    })?;
                ProviderConfig::Anthropic(config)
            }
            ProviderType::Bedrock => {
                let config: BedrockProviderConfig =
                    serde_json::from_value(helper.config).map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize Bedrock config: {e}"))
                    })?;
                ProviderConfig::Bedrock(config)
            }
            ProviderType::VertexAI => {
                let config: VertexAIProviderConfig = serde_json::from_value(helper.config)
                    .map_err(|e| {
                        D::Error::custom(format!("Failed to deserialize VertexAI config: {e}"))
                    })?;
                ProviderConfig::VertexAI(config)
            }
        };

        Ok(ProviderResponse {
            id: helper.id,
            name: helper.name,
            provider_type: helper.provider_type,
            config,
            enabled: helper.enabled,
            created_at: helper.created_at,
            updated_at: helper.updated_at,
        })
    }
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

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct LoggingConfigDto {
    #[schema(value_type = String, example = "debug")]
    pub level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct TracingConfigDto {
    #[schema(value_type = String, example = "http://api.traceloop.com/v1/tracing")]
    pub endpoint: String,
    #[schema(value_type = SecretObject, example = "tl_1234567890abcdef")]
    pub api_key: SecretObject,
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
            _ => Err(format!("Unknown plugin type: {s}")),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_logging_config_dto_serialization() {
        let config = LoggingConfigDto {
            level: "debug".to_string(),
        };

        let serialized = serde_json::to_value(&config).unwrap();
        assert_eq!(serialized, json!({"level": "debug"}));
    }

    #[test]
    fn test_logging_config_dto_deserialization() {
        let json_data = json!({"level": "info"});
        let config: LoggingConfigDto = serde_json::from_value(json_data).unwrap();
        assert_eq!(config.level, "info");
    }

    #[test]
    fn test_tracing_config_dto_serialization() {
        let config = TracingConfigDto {
            endpoint: "http://api.traceloop.com/v1/tracing".to_string(),
            api_key: SecretObject::literal("test-api-key".to_string()),
        };

        let serialized = serde_json::to_value(&config).unwrap();
        assert_eq!(
            serialized,
            json!({
                "endpoint": "http://api.traceloop.com/v1/tracing",
                "api_key": {
                    "type": "literal",
                    "value": "test-api-key"
                }
            })
        );
    }

    #[test]
    fn test_tracing_config_dto_deserialization() {
        let json_data = json!({
            "endpoint": "http://localhost:8080/v1/tracing",
            "api_key": {
                "type": "environment",
                "variable_name": "TRACING_API_KEY"
            }
        });
        let config: TracingConfigDto = serde_json::from_value(json_data).unwrap();

        assert_eq!(config.endpoint, "http://localhost:8080/v1/tracing");
        assert_eq!(
            config.api_key,
            SecretObject::environment("TRACING_API_KEY".to_string())
        );
    }

    #[test]
    fn test_tracing_config_dto_with_kubernetes_secret() {
        let config = TracingConfigDto {
            endpoint: "https://trace.example.com/v1/traces".to_string(),
            api_key: SecretObject::kubernetes(
                "tracing-secrets".to_string(),
                "api-key".to_string(),
                Some("monitoring".to_string()),
            ),
        };

        let serialized = serde_json::to_value(&config).unwrap();
        let deserialized: TracingConfigDto = serde_json::from_value(serialized).unwrap();

        assert_eq!(deserialized.endpoint, config.endpoint);
        assert_eq!(deserialized.api_key, config.api_key);
    }

    #[test]
    fn test_pipeline_plugin_config_dto_with_logging() {
        let plugin_config = PipelinePluginConfigDto {
            plugin_type: PluginType::Logging,
            config_data: json!({"level": "error"}),
            enabled: true,
            order_in_pipeline: 1,
        };

        let serialized = serde_json::to_value(&plugin_config).unwrap();
        let deserialized: PipelinePluginConfigDto = serde_json::from_value(serialized).unwrap();

        assert_eq!(deserialized.plugin_type, PluginType::Logging);
        assert_eq!(deserialized.config_data, json!({"level": "error"}));
        assert!(deserialized.enabled);
        assert_eq!(deserialized.order_in_pipeline, 1);
    }

    #[test]
    fn test_pipeline_plugin_config_dto_with_tracing() {
        let plugin_config = PipelinePluginConfigDto {
            plugin_type: PluginType::Tracing,
            config_data: json!({
                "endpoint": "http://trace.example.com/v1/traces",
                "api_key": {
                    "type": "literal",
                    "value": "secret-key"
                }
            }),
            enabled: true,
            order_in_pipeline: 2,
        };

        let serialized = serde_json::to_value(&plugin_config).unwrap();
        let deserialized: PipelinePluginConfigDto = serde_json::from_value(serialized).unwrap();

        assert_eq!(deserialized.plugin_type, PluginType::Tracing);
        assert!(deserialized.enabled);
        assert_eq!(deserialized.order_in_pipeline, 2);

        // Verify the config_data can be deserialized to TracingConfigDto
        let tracing_config: TracingConfigDto =
            serde_json::from_value(deserialized.config_data).unwrap();
        assert_eq!(
            tracing_config.endpoint,
            "http://trace.example.com/v1/traces"
        );
        assert_eq!(
            tracing_config.api_key,
            SecretObject::literal("secret-key".to_string())
        );
    }

    #[test]
    fn test_create_pipeline_request_with_logging_and_tracing() {
        let request = CreatePipelineRequestDto {
            name: "test-pipeline".to_string(),
            pipeline_type: "chat".to_string(),
            description: Some("Test pipeline with logging and tracing".to_string()),
            plugins: vec![
                PipelinePluginConfigDto {
                    plugin_type: PluginType::Logging,
                    config_data: json!({"level": "debug"}),
                    enabled: true,
                    order_in_pipeline: 1,
                },
                PipelinePluginConfigDto {
                    plugin_type: PluginType::Tracing,
                    config_data: json!({
                        "endpoint": "http://trace.example.com/v1/traces",
                        "api_key": {
                            "type": "environment",
                            "variable_name": "TRACE_API_KEY"
                        }
                    }),
                    enabled: true,
                    order_in_pipeline: 2,
                },
            ],
            enabled: true,
        };

        let serialized = serde_json::to_value(&request).unwrap();
        let deserialized: CreatePipelineRequestDto = serde_json::from_value(serialized).unwrap();

        assert_eq!(deserialized.name, "test-pipeline");
        assert_eq!(deserialized.plugins.len(), 2);

        // Verify first plugin (logging)
        let logging_plugin = &deserialized.plugins[0];
        assert_eq!(logging_plugin.plugin_type, PluginType::Logging);
        let logging_config: LoggingConfigDto =
            serde_json::from_value(logging_plugin.config_data.clone()).unwrap();
        assert_eq!(logging_config.level, "debug");

        // Verify second plugin (tracing)
        let tracing_plugin = &deserialized.plugins[1];
        assert_eq!(tracing_plugin.plugin_type, PluginType::Tracing);
        let tracing_config: TracingConfigDto =
            serde_json::from_value(tracing_plugin.config_data.clone()).unwrap();
        assert_eq!(
            tracing_config.endpoint,
            "http://trace.example.com/v1/traces"
        );
        assert_eq!(
            tracing_config.api_key,
            SecretObject::environment("TRACE_API_KEY".to_string())
        );
    }
}
