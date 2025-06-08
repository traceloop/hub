use crate::ai_models::registry::ModelRegistry;
use crate::config::hash::calculate_config_hash;
use crate::config::models::GatewayConfig;
use crate::providers::registry::ProviderRegistry;
use anyhow::{Context, Result};
use axum::Router;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

/// A snapshot of configuration state at a point in time
/// This reduces lock contention by capturing all needed data in one operation
#[derive(Clone)]
pub struct ConfigSnapshot {
    pub config: GatewayConfig,
    pub provider_registry: Arc<ProviderRegistry>,
    pub model_registry: Arc<ModelRegistry>,
}

// Removed RouterCache - using simplified approach with current_router

// Inner state that holds the frequently updated parts
struct InnerAppState {
    config: GatewayConfig,
    config_hash: u64,
    provider_registry: Arc<ProviderRegistry>,
    model_registry: Arc<ModelRegistry>,
}

impl InnerAppState {
    fn new(initial_config: GatewayConfig) -> Result<Self> {
        let config_hash = calculate_config_hash(&initial_config);
        let provider_registry_arc = Arc::new(ProviderRegistry::new(&initial_config.providers)?);
        let model_registry_arc = Arc::new(ModelRegistry::new(
            &initial_config.models,
            provider_registry_arc.clone(),
        )?);
        Ok(Self {
            config: initial_config,
            config_hash,
            provider_registry: provider_registry_arc,
            model_registry: model_registry_arc,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<RwLock<InnerAppState>>,
    // Simplified router cache - built once per configuration update
    current_router: Arc<RwLock<Router>>,
}

impl AppState {
    pub fn new(initial_config: GatewayConfig) -> Result<Self> {
        let inner_app_state =
            InnerAppState::new(initial_config).context("Failed to create initial InnerAppState")?;
        
        // Build initial router
        let initial_router = Self::build_router_for_config(&inner_app_state.config, &inner_app_state.provider_registry, &inner_app_state.model_registry);
        
        Ok(Self {
            inner: Arc::new(RwLock::new(inner_app_state)),
            current_router: Arc::new(RwLock::new(initial_router)),
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

    /// Get a snapshot of all configuration data in a single lock operation
    /// This is more efficient than calling individual getters when you need multiple values
    pub fn config_snapshot(&self) -> ConfigSnapshot {
        let guard = self.inner.read().unwrap();
        ConfigSnapshot {
            config: guard.config.clone(),
            provider_registry: guard.provider_registry.clone(),
            model_registry: guard.model_registry.clone(),
        }
    }

    /// Get the current router (always available)
    pub fn get_current_router(&self) -> Router {
        let guard = self.current_router.read().unwrap();
        guard.clone()
    }

    /// Update the current router (used internally during config updates)
    fn set_current_router(&self, router: Router) {
        *self.current_router.write().unwrap() = router;
        debug!("Router updated successfully");
    }

    /// Update configuration with change detection
    /// Only rebuilds router if configuration actually changed
    pub fn update_config(&self, new_config: GatewayConfig) -> Result<()> {
        // Check if configuration actually changed
        let current_hash = {
            let guard = self.inner.read().unwrap();
            guard.config_hash
        };
        
        let new_hash = calculate_config_hash(&new_config);
        
        if current_hash == new_hash {
            debug!("Configuration unchanged (hash: {}), skipping router rebuild", current_hash);
            return Ok(());
        }
        
        info!("Configuration changed (old hash: {}, new hash: {}), rebuilding router", current_hash, new_hash);
        
        // Validate configuration before applying
        if let Err(val_errors) = crate::config::validation::validate_gateway_config(&new_config) {
            return Err(anyhow::anyhow!("Invalid configuration: {:?}", val_errors));
        }
        
        // Build new registries
        let new_provider_registry = Arc::new(ProviderRegistry::new(&new_config.providers)?);
        let new_model_registry = Arc::new(ModelRegistry::new(&new_config.models, new_provider_registry.clone())?);
        
        // Build new router
        let new_router = Self::build_router_for_config(&new_config, &new_provider_registry, &new_model_registry);
        
        // Update everything atomically
        {
            let mut inner_guard = self.inner.write().unwrap();
            inner_guard.config = new_config;
            inner_guard.config_hash = new_hash;
            inner_guard.provider_registry = new_provider_registry;
            inner_guard.model_registry = new_model_registry;
        }
        
        // Update router
        self.set_current_router(new_router);
        
        info!("Configuration and router updated successfully");
        Ok(())
    }

    /// Static router building method that doesn't require self
    fn build_router_for_config(
        config: &GatewayConfig,
        _provider_registry: &Arc<ProviderRegistry>,
        model_registry: &Arc<ModelRegistry>,
    ) -> axum::Router {
        use crate::pipelines::pipeline::create_pipeline;
        use std::collections::HashMap;
        use tower::steer::Steer;
        use tracing::warn;

        let mut pipeline_idxs = HashMap::new();
        let mut routers = Vec::new();

        debug!(
            "Building router with {} pipelines",
            config.pipelines.len()
        );

        // Sort pipelines to ensure default is first
        let mut sorted_pipelines = config.pipelines.clone();
        sorted_pipelines.sort_by_key(|p| p.name != "default");

        for pipeline in sorted_pipelines {
            let name = pipeline.name.clone();
            debug!(
                "Adding pipeline '{}' to router at index {}",
                name,
                routers.len()
            );
            pipeline_idxs.insert(name, routers.len());
            routers.push(create_pipeline(&pipeline, model_registry));
        }

        // Always ensure we have at least one router
        if routers.is_empty() {
            warn!("No pipelines with routes found. Creating fallback router that returns 404.");
            let fallback_router = Self::create_no_config_router_static();
            routers.push(fallback_router);
            debug!("Fallback router created and added at index 0");
        }

        let routers_len = routers.len();
        debug!(
            "Router steering configured with {} total routers",
            routers_len
        );

        let pipeline_router = Steer::new(
            routers,
            move |req: &axum::extract::Request, _services: &[_]| {
                use tracing::warn;

                let pipeline_header = req
                    .headers()
                    .get("x-traceloop-pipeline")
                    .and_then(|h| h.to_str().ok());

                let index = pipeline_header
                    .and_then(|name| pipeline_idxs.get(name))
                    .copied()
                    .unwrap_or(0);

                if index >= routers_len {
                    warn!(
                        "Index {} is out of bounds (max: {}), using index 0",
                        index,
                        routers_len - 1
                    );
                    0
                } else {
                    index
                }
            },
        );

        axum::Router::new().nest_service("/", pipeline_router)
    }



    /// Static version of create_no_config_router
    fn create_no_config_router_static() -> axum::Router {
        // Use the centralized no-config router from routes module
        crate::routes::create_no_config_router()
    }

    // Legacy method for backward compatibility - delegates to new update_config method
    pub fn try_update_config_and_registries(&self, new_config: GatewayConfig) -> Result<()> {
        info!("Attempting to update live configuration and registries (providers: {}, models: {}, pipelines: {}).", 
              new_config.providers.len(), new_config.models.len(), new_config.pipelines.len());

        self.update_config(new_config)?;

        info!("Successfully updated live configuration and rebuilt registries.");
        Ok(())
    }
}
