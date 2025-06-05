use crate::ai_models::registry::ModelRegistry;
use crate::config::models::GatewayConfig;
use crate::providers::registry::ProviderRegistry;
use anyhow::{Result, Context};
use tracing::{info, debug};
use std::sync::{Arc, RwLock};

// Inner state that holds the frequently updated parts
struct InnerAppState {
    config: GatewayConfig,
    provider_registry: Arc<ProviderRegistry>,
    model_registry: Arc<ModelRegistry>,
}

impl InnerAppState {
    fn new(initial_config: GatewayConfig) -> Result<Self> {
        let provider_registry_arc = Arc::new(ProviderRegistry::new(&initial_config.providers)?);
        let model_registry_arc = Arc::new(ModelRegistry::new(
            &initial_config.models,
            provider_registry_arc.clone(),
        )?);
        Ok(Self {
            config: initial_config,
            provider_registry: provider_registry_arc,
            model_registry: model_registry_arc,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<InnerAppState>>,
}

impl AppState {
    pub fn new(initial_config: GatewayConfig) -> Result<Self> {
        let inner_app_state = InnerAppState::new(initial_config)
            .context("Failed to create initial InnerAppState")?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner_app_state)),
        })
    }

    // Public getters to access inner state fields safely
    pub fn current_config(&self) -> GatewayConfig {
        self.inner.read().unwrap().config.clone() // Clone to avoid holding lock
    }

    pub fn provider_registry(&self) -> Arc<ProviderRegistry> {
        self.inner.read().unwrap().provider_registry.clone()
    }

    pub fn model_registry(&self) -> Arc<ModelRegistry> {
        self.inner.read().unwrap().model_registry.clone()
    }
    
    // Assumes new_config is pre-validated by the caller (e.g., the poller)
    pub fn try_update_config_and_registries(&self, new_config: GatewayConfig) -> Result<()> {
        debug!("Attempting to update live configuration and registries.");

        let new_provider_registry_arc = Arc::new(ProviderRegistry::new(&new_config.providers)
            .context("Failed to create new provider registry during live update")?);
        let new_model_registry_arc = Arc::new(ModelRegistry::new(
            &new_config.models,
            new_provider_registry_arc.clone(),
        ).context("Failed to create new model registry during live update")?);

        let mut inner_guard = self.inner.write().unwrap();
        inner_guard.config = new_config;
        inner_guard.provider_registry = new_provider_registry_arc;
        inner_guard.model_registry = new_model_registry_arc;

        info!("Successfully updated live configuration and rebuilt registries.");
        Ok(())
    }
}
