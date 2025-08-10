use crate::types::{GatewayConfig, ModelConfig, Pipeline, PipelineType, PluginConfig, Provider};
use anyhow::{Result, anyhow};
use log::{error, warn};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use super::{
    super::dto::{
        LoggingConfigDto, ModelDefinitionResponse, ModelRouterConfigDto, PipelinePluginConfigDto,
        PipelineResponseDto,
        ProviderConfig, /*, OpenAIProviderConfig, AzureProviderConfig, BedrockProviderConfig*/
        ProviderResponse, TracingConfigDto,
    },
    model_definition_service::ModelDefinitionService,
    pipeline_service::PipelineService,
    provider_service::ProviderService,
    secret_resolver::SecretResolver,
};

// Helper function to convert serde_json::Value to String
// Handles simple types and serializes complex types to a JSON string.
fn convert_json_value_to_string(json_value: &JsonValue) -> String {
    match json_value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => String::new(), // Or a specific string like "null"
        JsonValue::Array(_) | JsonValue::Object(_) => serde_json::to_string(json_value)
            .unwrap_or_else(|e| {
                warn!("Failed to serialize complex JsonValue to string: {e}. Using empty string.");
                String::new()
            }),
    }
}

// Helper function to get JsonValue type as a string for logging
fn get_json_value_type_as_str(value: &JsonValue) -> &str {
    match value {
        JsonValue::Null => "Null",
        JsonValue::Bool(_) => "Bool",
        JsonValue::Number(_) => "Number",
        JsonValue::String(_) => "String",
        JsonValue::Array(_) => "Array",
        JsonValue::Object(_) => "Object",
    }
}

pub struct ConfigProviderService {
    provider_service: Arc<ProviderService>,
    model_definition_service: Arc<ModelDefinitionService>,
    pipeline_service: Arc<PipelineService>,
    secret_resolver: SecretResolver,
}

impl ConfigProviderService {
    pub fn new(
        provider_service: Arc<ProviderService>,
        model_definition_service: Arc<ModelDefinitionService>,
        pipeline_service: Arc<PipelineService>,
    ) -> Self {
        Self {
            provider_service,
            model_definition_service,
            pipeline_service,
            secret_resolver: SecretResolver::new(),
        }
    }

    pub async fn fetch_live_config(&self) -> Result<GatewayConfig> {
        debug!("Fetching live configuration from database...");
        let mut gateway_config = GatewayConfig::default();

        let db_providers = self
            .provider_service
            .list_providers()
            .await
            .map_err(|e| anyhow!("Failed to fetch providers from DB: {:?}", e))?;

        // Maps Provider DTO Uuid to its key (String) for model linking
        let mut provider_dto_id_to_key_map: HashMap<Uuid, String> = HashMap::new();

        for p_dto in db_providers.into_iter().filter(|p| p.enabled) {
            // Store the original DTO ID for mapping before transforming
            let original_dto_id = p_dto.id;
            match self.transform_provider_dto(p_dto).await {
                Ok(core_provider) => {
                    // Use the DTO's id for the map key, and core_provider's key for the value
                    provider_dto_id_to_key_map.insert(original_dto_id, core_provider.key.clone());
                    gateway_config.providers.push(core_provider);
                }
                Err(e) => error!(
                    "Failed to transform provider DTO with ID {original_dto_id}: {e:?}. Skipping."
                ),
            }
        }

        let db_models = self
            .model_definition_service
            .list_model_definitions()
            .await
            .map_err(|e| anyhow!("Failed to fetch model definitions from DB: {:?}", e))?;
        for m_dto in db_models.into_iter().filter(|m| m.enabled) {
            // Use the provider_id from the model DTO (which is a Uuid) to lookup in the map
            match Self::transform_model_dto(m_dto, &provider_dto_id_to_key_map) {
                Ok(core_model) => gateway_config.models.push(core_model),
                Err(e) => error!("Failed to transform model DTO: {e:?}. Skipping."),
            }
        }

        let db_pipelines = self
            .pipeline_service
            .list_pipelines()
            .await
            .map_err(|e| anyhow!("Failed to fetch pipelines from DB: {:?}", e))?;
        for pl_dto in db_pipelines.into_iter().filter(|pl| pl.enabled) {
            match self.transform_pipeline_dto(pl_dto).await {
                Ok(core_pipeline) => gateway_config.pipelines.push(core_pipeline),
                Err(e) => error!("Failed to transform pipeline DTO: {e:?}. Skipping."),
            }
        }

        debug!("Successfully fetched and transformed live configuration.");
        Ok(gateway_config)
    }

    async fn transform_provider_dto(&self, dto: ProviderResponse) -> Result<Provider> {
        let mut params = HashMap::new();
        let api_key_from_dto = match dto.config {
            ProviderConfig::OpenAI(c) => {
                if let Some(org_id) = c.organization_id {
                    params.insert("organization_id".to_string(), org_id);
                }
                Some(self.secret_resolver.resolve_secret(&c.api_key).await?)
            }
            ProviderConfig::Azure(c) => {
                params.insert("resource_name".to_string(), c.resource_name);
                params.insert("api_version".to_string(), c.api_version);
                if let Some(base_url) = c.base_url {
                    params.insert("base_url".to_string(), base_url);
                }
                Some(self.secret_resolver.resolve_secret(&c.api_key).await?)
            }
            ProviderConfig::Anthropic(c) => {
                Some(self.secret_resolver.resolve_secret(&c.api_key).await?)
            }
            ProviderConfig::Bedrock(c) => {
                params.insert("region".to_string(), c.region.clone());
                if let Some(access_key) = &c.aws_access_key_id {
                    let resolved_key = self.secret_resolver.resolve_secret(access_key).await?;
                    params.insert("AWS_ACCESS_KEY_ID".to_string(), resolved_key);
                }
                if let Some(secret) = &c.aws_secret_access_key {
                    let resolved_secret = self.secret_resolver.resolve_secret(secret).await?;
                    params.insert("AWS_SECRET_ACCESS_KEY".to_string(), resolved_secret);
                }
                if let Some(token) = &c.aws_session_token {
                    let resolved_token = self.secret_resolver.resolve_secret(token).await?;
                    params.insert("AWS_SESSION_TOKEN".to_string(), resolved_token);
                }
                if let Some(use_iam_role) = c.use_iam_role {
                    params.insert("use_iam_role".to_string(), use_iam_role.to_string());
                }
                if let Some(inference_profile_id) = c.inference_profile_id {
                    params.insert("inference_profile_id".to_string(), inference_profile_id);
                }
                None
            }
            ProviderConfig::VertexAI(c) => {
                params.insert("project_id".to_string(), c.project_id);
                params.insert("location".to_string(), c.location);
                if let Some(credentials_path) = c.credentials_path {
                    params.insert("credentials_path".to_string(), credentials_path);
                }
                if let Some(api_key) = &c.api_key {
                    Some(self.secret_resolver.resolve_secret(api_key).await?)
                } else {
                    None
                }
            }
        };

        Ok(Provider {
            key: dto.name,
            r#type: dto.provider_type,
            api_key: api_key_from_dto.unwrap_or_default(),
            params,
        })
    }

    fn transform_model_dto(
        dto: ModelDefinitionResponse,
        provider_dto_id_to_key_map: &HashMap<Uuid, String>,
    ) -> Result<ModelConfig> {
        let provider_key = provider_dto_id_to_key_map
            .get(&dto.provider.id)
            .ok_or_else(|| {
                anyhow!(
                    "Provider key not found for provider ID {} (model key '{}')",
                    dto.provider.id,
                    dto.key
                )
            })?
            .clone();

        let mut params = HashMap::new();
        match dto.config_details {
            JsonValue::Object(map) => {
                for (k, v) in map {
                    params.insert(k, convert_json_value_to_string(&v));
                }
            }
            JsonValue::Null => {}
            _ => {
                warn!(
                    "Model '{}' config_details is not a JSON object (type: {}). Cannot convert to params map.",
                    dto.key,
                    get_json_value_type_as_str(&dto.config_details)
                );
            }
        }

        Ok(ModelConfig {
            key: dto.key,
            r#type: dto.model_type,
            provider: provider_key,
            params,
        })
    }

    async fn transform_pipeline_dto(&self, dto: PipelineResponseDto) -> Result<Pipeline> {
        let core_pipeline_type = match dto.pipeline_type.to_lowercase().as_str() {
            "chat" => PipelineType::Chat,
            "completion" => PipelineType::Completion,
            "embeddings" => PipelineType::Embeddings,
            _ => return Err(anyhow!("Unsupported pipeline type: {}", dto.pipeline_type)),
        };

        let mut core_plugins = Vec::new();
        for plugin_dto in dto.plugins.into_iter().filter(|p| p.enabled) {
            match self.transform_plugin_dto(plugin_dto).await {
                Ok(p) => core_plugins.push(p),
                Err(e) => error!(
                    "Failed to transform plugin DTO for pipeline '{}': {:?}. Skipping.",
                    dto.name, e
                ),
            }
        }

        Ok(Pipeline {
            name: dto.name,
            r#type: core_pipeline_type,
            plugins: core_plugins,
        })
    }

    async fn transform_plugin_dto(&self, dto: PipelinePluginConfigDto) -> Result<PluginConfig> {
        match dto.plugin_type {
            super::super::dto::PluginType::ModelRouter => {
                let mr_config: ModelRouterConfigDto = serde_json::from_value(dto.config_data)
                    .map_err(|e| {
                        anyhow!(
                            "Failed to deserialize ModelRouterConfigDto for plugin type '{:?}': {e}",
                            dto.plugin_type
                        )
                    })?;

                let model_keys = mr_config.models.into_iter().map(|m| m.key).collect();
                Ok(PluginConfig::ModelRouter { models: model_keys })
            }
            super::super::dto::PluginType::Logging => {
                let logging_config: LoggingConfigDto = serde_json::from_value(dto.config_data)
                    .map_err(|e| {
                        anyhow!(
                            "Failed to deserialize LoggingConfigDto for plugin type '{:?}': {e}",
                            dto.plugin_type
                        )
                    })?;

                Ok(PluginConfig::Logging {
                    level: logging_config.level,
                })
            }
            super::super::dto::PluginType::Tracing => {
                let tracing_config: TracingConfigDto = serde_json::from_value(dto.config_data)
                    .map_err(|e| {
                        anyhow!(
                            "Failed to deserialize TracingConfigDto for plugin type '{:?}': {e}",
                            dto.plugin_type
                        )
                    })?;

                let resolved_api_key = self
                    .secret_resolver
                    .resolve_secret(&tracing_config.api_key)
                    .await?;

                Ok(PluginConfig::Tracing {
                    endpoint: tracing_config.endpoint,
                    api_key: resolved_api_key,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::management::dto::{
        LoggingConfigDto, PipelinePluginConfigDto, PluginType, SecretObject, TracingConfigDto,
    };
    use serde_json::json;

    #[test]
    fn test_logging_config_dto_deserialization() {
        let json_data = json!({"level": "info"});
        let config: LoggingConfigDto = serde_json::from_value(json_data).unwrap();
        assert_eq!(config.level, "info");
    }

    #[test]
    fn test_tracing_config_dto_deserialization() {
        let json_data = json!({
            "endpoint": "http://trace.example.com/v1/traces",
            "api_key": {
                "type": "literal",
                "value": "test-key"
            }
        });
        let config: TracingConfigDto = serde_json::from_value(json_data).unwrap();
        assert_eq!(config.endpoint, "http://trace.example.com/v1/traces");
        assert_eq!(
            config.api_key,
            SecretObject::literal("test-key".to_string())
        );
    }

    #[test]
    fn test_invalid_logging_config_deserialization() {
        let json_data = json!({"invalid_field": "value"});
        let result: Result<LoggingConfigDto, _> = serde_json::from_value(json_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_tracing_config_deserialization() {
        let json_data = json!({"endpoint": "http://trace.example.com"});
        let result: Result<TracingConfigDto, _> = serde_json::from_value(json_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_config_dto_validation() {
        // Test valid logging plugin config
        let logging_plugin = PipelinePluginConfigDto {
            plugin_type: PluginType::Logging,
            config_data: json!({"level": "debug"}),
            enabled: true,
            order_in_pipeline: 1,
        };

        let logging_config: LoggingConfigDto =
            serde_json::from_value(logging_plugin.config_data).unwrap();
        assert_eq!(logging_config.level, "debug");

        // Test valid tracing plugin config
        let tracing_plugin = PipelinePluginConfigDto {
            plugin_type: PluginType::Tracing,
            config_data: json!({
                "endpoint": "http://trace.example.com/v1/traces",
                "api_key": {
                    "type": "environment",
                    "variable_name": "TRACE_KEY"
                }
            }),
            enabled: true,
            order_in_pipeline: 2,
        };

        let tracing_config: TracingConfigDto =
            serde_json::from_value(tracing_plugin.config_data).unwrap();
        assert_eq!(
            tracing_config.endpoint,
            "http://trace.example.com/v1/traces"
        );
        assert_eq!(
            tracing_config.api_key,
            SecretObject::environment("TRACE_KEY".to_string())
        );
    }
}
