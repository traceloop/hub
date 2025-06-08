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
        let guard = self.cached_pipeline_router.read().unwrap();
        match guard.as_ref() {
            Some(router) => {
                debug!("Retrieved cached pipeline router");
                Some(router.clone())
            }
            None => {
                debug!("No cached pipeline router found");
                None
            }
        }
    }

    pub fn set_cached_pipeline_router(&self, router: Router) {
        *self.cached_pipeline_router.write().unwrap() = Some(router);
        info!("Pipeline router cached successfully");
    }

    pub fn invalidate_cached_router(&self) {
        let mut guard = self.cached_pipeline_router.write().unwrap();
        let had_cache = guard.is_some();
        *guard = None;
        info!("Pipeline router cache invalidated (had cached router: {}) - will rebuild on next request", had_cache);
    }

    /// Rebuilds the router immediately and caches it
    pub fn rebuild_pipeline_router_now(&self) -> Result<()> {
        info!("Force rebuilding pipeline router with current configuration");
        
        // Build router directly using internal method to avoid Arc wrapping
        let router = self.build_router_internal();
        
        // Cache the new router
        self.set_cached_pipeline_router(router);
        info!("Pipeline router rebuilt and cached successfully");
        
        Ok(())
    }

    /// Internal router building method to avoid circular dependencies and Arc wrapping
    fn build_router_internal(&self) -> axum::Router {
        use crate::pipelines::pipeline::create_pipeline;
        use std::collections::HashMap;
        use tower::steer::Steer;
        use tracing::{info, warn};

        let mut pipeline_idxs = HashMap::new();
        let mut routers = Vec::new();

        // Get current configuration
        let current_config = self.current_config();
        let model_registry = self.model_registry();

        info!("Building router with {} pipelines", current_config.pipelines.len());

        // Sort pipelines to ensure default is first
        let mut sorted_pipelines = current_config.pipelines.clone();
        sorted_pipelines.sort_by_key(|p| p.name != "default");

        for pipeline in sorted_pipelines {
            let name = pipeline.name.clone();
            info!("Adding pipeline '{}' to router at index {}", name, routers.len());
            pipeline_idxs.insert(name, routers.len());
            routers.push(create_pipeline(&pipeline, &model_registry));
        }

        // Always ensure we have at least one router
        if routers.is_empty() {
            warn!("No pipelines with routes found. Creating fallback router that returns 503.");
            let fallback_router = self.create_no_config_router();
            routers.push(fallback_router);
            info!("Fallback router created and added at index 0");
        }

        let routers_len = routers.len();
        info!("Router steering configured with {} total routers", routers_len);

        let pipeline_idxs_clone = pipeline_idxs.clone();

        let pipeline_router = Steer::new(routers, move |req: &axum::extract::Request, _services: &[_]| {
            use tracing::{info, warn};
            
            let pipeline_header = req.headers()
                .get("x-traceloop-pipeline")
                .and_then(|h| h.to_str().ok());
            
            let index = pipeline_header
                .and_then(|name| pipeline_idxs.get(name))
                .copied()
                .unwrap_or(0);
            
            info!("Request URI: {}, Pipeline header: {:?}, Available pipelines: {:?}, Routing to index: {}/{}", 
                   req.uri(), pipeline_header, pipeline_idxs_clone.keys().collect::<Vec<_>>(), index, routers_len - 1);
            
            if index >= routers_len {
                warn!("Index {} is out of bounds (max: {}), using index 0", index, routers_len - 1);
                0
            } else {
                index
            }
        });

        axum::Router::new().nest_service("/", pipeline_router)
    }

    /// Creates a router that handles requests when no configuration is available
    fn create_no_config_router(&self) -> axum::Router {
        use axum::{routing::post, Json, http::StatusCode};
        use tracing::{info, warn};

        async fn no_config_handler() -> Result<Json<serde_json::Value>, StatusCode> {
            warn!("No configuration available - returning 503 Service Unavailable");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }

        info!("Creating no-config fallback router");
        axum::Router::new()
            .route("/chat/completions", post(no_config_handler))
            .route("/completions", post(no_config_handler))  
            .route("/embeddings", post(no_config_handler))
            .fallback(no_config_handler)
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
