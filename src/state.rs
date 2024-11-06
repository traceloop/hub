use crate::ai_models::registry::ModelRegistry;
use crate::config::models::Config;
use crate::providers::registry::ProviderRegistry;
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub http_client: Client,
    pub provider_registry: Arc<ProviderRegistry>,
    pub model_registry: Arc<ModelRegistry>,
}

impl AppState {
    pub fn new(config: Config) -> Result<Self> {
        let provider_registry = Arc::new(ProviderRegistry::new(&config.providers)?);
        let model_registry = Arc::new(ModelRegistry::new(
            &config.models,
            provider_registry.clone(),
        )?);

        Ok(Self {
            config: Arc::new(config),
            http_client: Client::new(),
            provider_registry,
            model_registry,
        })
    }
}
