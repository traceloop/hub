use crate::ai_models::registry::ModelRegistry;
use crate::config::models::GatewayConfig;
use crate::providers::registry::ProviderRegistry;
use anyhow::{Result, Context};
use tracing::{info, debug, warn};
use std::sync::{Arc, RwLock};
use axum::Router;

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
    // We'll store an optional cached router that gets rebuilt when config changes
    cached_pipeline_router: Arc<RwLock<Option<Router>>>,
}

impl AppState {
    pub fn new(initial_config: GatewayConfig) -> Result<Self> {
        let inner_app_state = InnerAppState::new(initial_config)
            .context("Failed to create initial InnerAppState")?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner_app_state)),
            cached_pipeline_router: Arc::new(RwLock::new(None)),
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

    pub fn get_cached_pipeline_router(&self) -> Option<Router> {
        let cached = self.cached_pipeline_router.read().unwrap().clone();
        match &cached {
            Some(_) => debug!("Retrieved cached pipeline router"),
            None => debug!("No cached pipeline router found"),
        }
        cached
    }

    pub fn set_cached_pipeline_router(&self, router: Router) {
        *self.cached_pipeline_router.write().unwrap() = Some(router);
        info!("Pipeline router cached successfully");
    }

    pub fn invalidate_cached_router(&self) {
        let had_cache = self.cached_pipeline_router.read().unwrap().is_some();
        *self.cached_pipeline_router.write().unwrap() = None;
        info!("Pipeline router cache invalidated (had cached router: {}) - will rebuild on next request", had_cache);
    }

    /// Rebuilds the router immediately and caches it
    pub fn rebuild_pipeline_router_now(&self) -> Result<()> {
        info!("Force rebuilding pipeline router with current configuration");
        
        // We need to create a temporary Arc to pass to the router builder
        // Since AppState is Clone, we can create this efficiently
        let temp_arc = Arc::new(self.clone());
        
        // Import here to avoid circular dependencies
        let router = crate::routes::build_pipeline_router_from_config_direct(temp_arc);
        
        // Cache the new router
        self.set_cached_pipeline_router(router);
        info!("Pipeline router rebuilt and cached successfully");
        
        Ok(())
    }
    
    // Assumes new_config is pre-validated by the caller (e.g., the poller)
    pub fn try_update_config_and_registries(&self, new_config: GatewayConfig) -> Result<()> {
        info!("Attempting to update live configuration and registries (providers: {}, models: {}, pipelines: {}).", 
              new_config.providers.len(), new_config.models.len(), new_config.pipelines.len());

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
        
        // Drop the write lock before invalidating cache to avoid holding multiple locks
        drop(inner_guard);

        // Invalidate the cached router so it gets rebuilt on next request
        self.invalidate_cached_router();

        // Optionally rebuild immediately for better performance
        if let Err(e) = self.rebuild_pipeline_router_now() {
            warn!("Failed to rebuild router immediately: {}. Router will be rebuilt lazily on next request.", e);
        }

        info!("Successfully updated live configuration and rebuilt registries.");
        Ok(())
    }
}
