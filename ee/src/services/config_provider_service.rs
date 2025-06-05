use anyhow::{Result, anyhow};
use hub_gateway_core_types::{GatewayConfig, Provider, ModelConfig, Pipeline, PipelineType, PluginConfig};
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use serde_json::Value as JsonValue;

use crate::{
    dto::{ProviderResponse, ModelDefinitionResponse, PipelineResponseDto, ModelRouterConfigDto, PipelinePluginConfigDto, ProviderConfig as EeProviderConfig /*, OpenAIProviderConfig, AzureProviderConfig, BedrockProviderConfig*/},
    services::{provider_service::ProviderService, model_definition_service::ModelDefinitionService, pipeline_service::PipelineService},
    // errors::ApiError, // Assuming ApiError can be converted to anyhow::Error or handled
};

// Helper function to convert serde_json::Value to String
// Handles simple types and serializes complex types to a JSON string.
fn convert_json_value_to_string(json_value: &JsonValue) -> String {
    match json_value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => String::new(), // Or a specific string like "null"
        JsonValue::Array(_) | JsonValue::Object(_) => {
            serde_json::to_string(json_value).unwrap_or_else(|e| {
                warn!("Failed to serialize complex JsonValue to string: {}. Using empty string.", e);
                String::new()
            })
        }
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
        }
    }

    pub async fn fetch_live_config(&self) -> Result<GatewayConfig> {
        info!("Fetching live configuration from database...");
        let mut gateway_config = GatewayConfig::default();

        let ee_providers = self.provider_service.list_providers().await
            .map_err(|e| anyhow!("Failed to fetch providers from DB: {:?}", e))?;
        
        // Maps Provider DTO Uuid to its key (String) for model linking
        let mut provider_dto_id_to_key_map: HashMap<Uuid, String> = HashMap::new();

        for p_dto in ee_providers.into_iter().filter(|p| p.enabled) {
            // Store the original DTO ID for mapping before transforming
            let original_dto_id = p_dto.id;
            match Self::transform_provider_dto(p_dto) {
                Ok(core_provider) => {
                    // Use the DTO's id for the map key, and core_provider's key for the value
                    provider_dto_id_to_key_map.insert(original_dto_id, core_provider.key.clone());
                    gateway_config.providers.push(core_provider);
                }
                Err(e) => error!("Failed to transform provider DTO with ID {}: {:?}. Skipping.", original_dto_id, e),
            }
        }

        let ee_models = self.model_definition_service.list_model_definitions().await
            .map_err(|e| anyhow!("Failed to fetch model definitions from DB: {:?}", e))?;
        for m_dto in ee_models.into_iter().filter(|m| m.enabled) {
            // Use the provider_id from the model DTO (which is a Uuid) to lookup in the map
            match Self::transform_model_dto(m_dto, &provider_dto_id_to_key_map) {
                Ok(core_model) => gateway_config.models.push(core_model),
                Err(e) => error!("Failed to transform model DTO: {:?}. Skipping.", e),
            }
        }
        
        let ee_pipelines = self.pipeline_service.list_pipelines().await
            .map_err(|e| anyhow!("Failed to fetch pipelines from DB: {:?}", e))?;
        for pl_dto in ee_pipelines.into_iter().filter(|pl| pl.enabled) {
            match Self::transform_pipeline_dto(pl_dto) {
                Ok(core_pipeline) => gateway_config.pipelines.push(core_pipeline),
                Err(e) => error!("Failed to transform pipeline DTO: {:?}. Skipping.", e),
            }
        }
        
        info!("Successfully fetched and transformed live configuration.");
        Ok(gateway_config)
    }

    fn transform_provider_dto(dto: ProviderResponse) -> Result<Provider> {
        let mut params = HashMap::new();
        let api_key_from_dto = match dto.config {
            EeProviderConfig::OpenAI(c) => {
                if let Some(org_id) = c.organization_id {
                    params.insert("organization_id".to_string(), org_id);
                }
                Some(c.api_key)
            }
            EeProviderConfig::Azure(c) => {
                params.insert("resource_name".to_string(), c.resource_name);
                params.insert("api_version".to_string(), c.api_version);
                Some(c.api_key)
            }
            EeProviderConfig::Bedrock(c) => {
                params.insert("region".to_string(), c.region);
                if let Some(secret) = c.aws_secret_access_key {
                    params.insert("aws_secret_access_key".to_string(), secret);
                }
                if let Some(token) = c.aws_session_token {
                    params.insert("aws_session_token".to_string(), token);
                }
                c.aws_access_key_id.clone()
            }
        };

        Ok(Provider {
            key: dto.name,
            r#type: dto.provider_type.to_string(),
            api_key: api_key_from_dto.unwrap_or_default(),
            params,
        })
    }

    fn transform_model_dto(dto: ModelDefinitionResponse, provider_dto_id_to_key_map: &HashMap<Uuid, String>) -> Result<ModelConfig> {
        let provider_key = provider_dto_id_to_key_map.get(&dto.provider.id)
            .ok_or_else(|| {
                anyhow!(
                    "Provider key not found for provider ID {} (model key '{}')",
                    dto.provider.id, dto.key
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

    fn transform_pipeline_dto(dto: PipelineResponseDto) -> Result<Pipeline> {
        let core_pipeline_type = match dto.pipeline_type.to_lowercase().as_str() {
            "chat" => PipelineType::Chat,
            "completion" => PipelineType::Completion,
            "embeddings" => PipelineType::Embeddings,
            _ => return Err(anyhow!("Unsupported pipeline type: {}", dto.pipeline_type)),
        };

        let mut core_plugins = Vec::new();
        for plugin_dto in dto.plugins.into_iter().filter(|p| p.enabled) {
            match Self::transform_plugin_dto(plugin_dto) {
                Ok(p) => core_plugins.push(p),
                Err(e) => error!("Failed to transform plugin DTO for pipeline '{}': {:?}. Skipping.", dto.name, e),
            }
        }

        Ok(Pipeline {
            name: dto.name,
            r#type: core_pipeline_type,
            plugins: core_plugins,
        })
    }

    fn transform_plugin_dto(dto: PipelinePluginConfigDto) -> Result<PluginConfig> {
        match dto.plugin_type {
            crate::dto::PluginType::ModelRouter => {
                let mr_config: ModelRouterConfigDto = serde_json::from_value(dto.config_data)
                    .map_err(|e| anyhow!("Failed to deserialize ModelRouterConfigDto for plugin type '{:?}': {}", dto.plugin_type, e))?;
                
                let model_keys = mr_config.models.into_iter().map(|m| m.key).collect();
                Ok(PluginConfig::ModelRouter { models: model_keys })
            }
            crate::dto::PluginType::Logging => {
                let level = dto.config_data.get("level").and_then(|v| v.as_str()).unwrap_or("warning").to_string();
                Ok(PluginConfig::Logging { level })
            }
            crate::dto::PluginType::Tracing => {
                let endpoint = dto.config_data.get("endpoint").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing endpoint for tracing plugin"))?.to_string();
                let api_key = dto.config_data.get("api_key").and_then(|v| v.as_str()).map(String::from);
                Ok(PluginConfig::Tracing { endpoint, api_key })
            }
        }
    }
} 