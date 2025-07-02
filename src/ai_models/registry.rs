use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use super::instance::ModelInstance;
use crate::config::models::ModelConfig;
use crate::providers::registry::ProviderRegistry;
use crate::models::responses::{ModelInfoResponse, ModelListResponse};

#[derive(Clone)]
pub struct ModelRegistry {
    models: HashMap<String, Arc<ModelInstance>>,
}

impl ModelRegistry {
    pub fn new(
        model_configs: &[ModelConfig],
        provider_registry: Arc<ProviderRegistry>,
    ) -> Result<Self> {
        let mut models = HashMap::new();

        for config in model_configs {
            if let Some(provider) = provider_registry.get(&config.provider) {
                let model = Arc::new(ModelInstance {
                    name: config.key.clone(),
                    model_type: config.r#type.clone(),
                    provider,
                    config: config.clone(),
                });

                models.insert(config.key.clone(), model);
            }
        }

        Ok(Self { models })
    }

    pub fn get(&self, name: &str) -> Option<Arc<ModelInstance>> {
        self.models.get(name).cloned()
    }

    pub fn get_filtered_model_info(&self, allowed_models: &[String]) -> ModelListResponse {
        ModelListResponse {
            object: "list".to_string(),
            data: self.models
                .values()
                .filter(|model| allowed_models.contains(&model.name))
                .map(|model| ModelInfoResponse {
                    id: model.name.clone(),
                    object: "model".to_string(),
                    owned_by: model.provider.key(),
                })
                .collect()
        }
    }
}
