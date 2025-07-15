use crate::ai_models::registry::ModelRegistry;
use crate::config::hash::calculate_config_hash;
use crate::config::models::GatewayConfig;
use crate::providers::registry::ProviderRegistry;
use anyhow::{Context, Result};
use axum::{Router, body::Body, extract::Request};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use tracing::{debug, warn};

const PIPELINE_HEADER: &str = "x-traceloop-pipeline";

const DEFAULT_PIPELINE_NAME: &str = "default";

const FALLBACK_PIPELINE_NAME: &str = "fallback";

/// A snapshot of configuration state at a point in time
/// This reduces lock contention by capturing all needed data in one operation
/// NOTE: This struct is only used in tests and should not be used in production code
#[derive(Clone)]
pub struct ConfigSnapshot {
    pub config: GatewayConfig,
    pub provider_registry: Arc<ProviderRegistry>,
    pub model_registry: Arc<ModelRegistry>,
}

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
    current_router: Arc<RwLock<Arc<Router>>>,
}

impl AppState {
    pub fn new(initial_config: GatewayConfig) -> Result<Self> {
        let inner_app_state =
            InnerAppState::new(initial_config).context("Failed to create initial InnerAppState")?;

        // Build initial router
        let initial_router = Self::build_router_for_config(
            &inner_app_state.config,
            &inner_app_state.provider_registry,
            &inner_app_state.model_registry,
        );

        Ok(Self {
            inner: Arc::new(RwLock::new(inner_app_state)),
            current_router: Arc::new(RwLock::new(Arc::new(initial_router))),
        })
    }

    // Public getters to access inner state fields safely
    // Note: These methods use unwrap() for lock acquisition, which is acceptable
    // since lock poisoning is extremely rare in practice

    /// Get the current configuration
    /// NOTE: This method is only used in tests and should not be used in production code
    pub fn current_config(&self) -> GatewayConfig {
        self.inner.read().unwrap().config.clone() // Clone to avoid holding lock
    }

    /// Get a snapshot of all configuration data in a single lock operation
    /// This is more efficient than calling individual getters when you need multiple values
    /// NOTE: This method is only used in tests and should not be used in production code
    pub fn config_snapshot(&self) -> ConfigSnapshot {
        let guard = self.inner.read().unwrap();
        ConfigSnapshot {
            config: guard.config.clone(),
            provider_registry: guard.provider_registry.clone(),
            model_registry: guard.model_registry.clone(),
        }
    }

    pub fn get_current_router(&self) -> Arc<Router> {
        let guard = self.current_router.read().unwrap();
        Arc::clone(&guard)
    }

    fn set_current_router(&self, router: Router) {
        *self.current_router.write().unwrap() = Arc::new(router);
        debug!("Router updated successfully");
    }

    /// Update configuration with change detection
    /// Only rebuilds router if configuration actually changed
    pub fn update_config(&self, new_config: GatewayConfig) -> Result<()> {
        let current_hash = {
            let guard = self.inner.read().unwrap();
            guard.config_hash
        };

        let new_hash = calculate_config_hash(&new_config);

        if current_hash == new_hash {
            debug!(
                "Configuration unchanged (hash: {}), skipping router rebuild",
                current_hash
            );
            return Ok(());
        }

        debug!(
            "Configuration changed (old hash: {}, new hash: {}), rebuilding router",
            current_hash, new_hash
        );

        if let Err(val_errors) = crate::config::validation::validate_gateway_config(&new_config) {
            return Err(anyhow::anyhow!("Invalid configuration: {:?}", val_errors));
        }

        let new_provider_registry = Arc::new(ProviderRegistry::new(&new_config.providers)?);
        let new_model_registry = Arc::new(ModelRegistry::new(
            &new_config.models,
            new_provider_registry.clone(),
        )?);

        let new_router =
            Self::build_router_for_config(&new_config, &new_provider_registry, &new_model_registry);

        {
            let mut inner_guard = self.inner.write().unwrap();
            inner_guard.config = new_config;
            inner_guard.config_hash = new_hash;
            inner_guard.provider_registry = new_provider_registry;
            inner_guard.model_registry = new_model_registry;
        }

        self.set_current_router(new_router);

        debug!("Configuration and router updated successfully");
        Ok(())
    }

    fn build_router_for_config(
        config: &GatewayConfig,
        _provider_registry: &Arc<ProviderRegistry>,
        model_registry: &Arc<ModelRegistry>,
    ) -> axum::Router {
        use crate::pipelines::pipeline::create_pipeline;

        debug!("Building router with {} pipelines", config.pipelines.len());

        let (default_pipeline, other_pipelines): (Vec<_>, Vec<_>) = config
            .pipelines
            .iter()
            .partition(|p| p.name == DEFAULT_PIPELINE_NAME);

        let mut pipeline_routers = Vec::with_capacity(config.pipelines.len());
        let mut pipeline_names = Vec::with_capacity(config.pipelines.len());
        let default_pipeline_idx = 0; // Default is always first

        if let Some(default_pipeline) = default_pipeline.first() {
            debug!(
                "Adding default pipeline '{}' to router at index 0",
                default_pipeline.name
            );
            let pipeline_router = create_pipeline(default_pipeline, model_registry);
            pipeline_routers.push(pipeline_router);
            pipeline_names.push(default_pipeline.name.clone());
        }

        for (idx, pipeline) in other_pipelines.iter().enumerate() {
            let name = &pipeline.name;
            debug!("Adding pipeline '{}' to router at index {}", name, idx + 1);

            let pipeline_router = create_pipeline(pipeline, model_registry);
            pipeline_routers.push(pipeline_router);
            pipeline_names.push(name.clone());
        }

        if pipeline_routers.is_empty() {
            warn!("No pipelines with routes found. Creating fallback router that returns 404.");
            let fallback_router = Self::create_no_config_router_static();
            pipeline_routers.push(fallback_router);
            pipeline_names.push(FALLBACK_PIPELINE_NAME.to_string());
            debug!("Fallback router created");
        }

        debug!(
            "Router steering configured with {} total pipelines, default: '{}'",
            pipeline_routers.len(),
            pipeline_names[default_pipeline_idx]
        );

        Self::create_pipeline_steering_router(
            pipeline_routers,
            pipeline_names,
            default_pipeline_idx,
        )
    }

    fn create_no_config_router_static() -> axum::Router {
        crate::routes::create_no_config_router()
    }

    fn create_pipeline_steering_router(
        pipeline_routers: Vec<Router>,
        pipeline_names: Vec<String>,
        default_pipeline_idx: usize,
    ) -> Router {
        if pipeline_routers.len() == 1 {
            return pipeline_routers
                .into_iter()
                .next()
                .expect("Single pipeline should exist");
        }

        let default_pipeline_name = pipeline_names
            .get(default_pipeline_idx)
            .expect("Default pipeline index should be valid")
            .clone();

        let pipeline_map: HashMap<String, Arc<Router>> = pipeline_names
            .into_iter()
            .zip(pipeline_routers)
            .map(|(name, router)| (name, Arc::new(router)))
            .collect();

        let steering_service = PipelineSteeringService::new(pipeline_map, default_pipeline_name);

        Router::new().fallback_service(steering_service)
    }
}

#[derive(Clone)]
pub struct PipelineSteeringService {
    pipeline_routers: HashMap<String, Arc<Router>>,
    default_pipeline: Arc<Router>,
}

impl PipelineSteeringService {
    pub fn new(
        pipeline_routers: HashMap<String, Arc<Router>>,
        default_pipeline_name: String,
    ) -> Self {
        let default_pipeline = pipeline_routers
            .get(&default_pipeline_name)
            .expect("Default pipeline should exist in pipeline_routers")
            .clone();

        Self {
            pipeline_routers,
            default_pipeline,
        }
    }

    /// Extract pipeline name from header without unnecessary string allocations
    fn get_pipeline_name_from_header<'a>(&self, request: &'a Request<Body>) -> Option<&'a str> {
        request
            .headers()
            .get(PIPELINE_HEADER)
            .and_then(|header| header.to_str().ok())
    }
}

impl tower::Service<Request<Body>> for PipelineSteeringService {
    type Response = axum::response::Response;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let pipeline_name = self.get_pipeline_name_from_header(&request);

        let router = if let Some(name) = pipeline_name {
            debug!("Routing request to pipeline: '{}'", name);

            self.pipeline_routers
                .get(name)
                .unwrap_or_else(|| {
                    debug!(
                        "Pipeline '{}' not found, falling back to default pipeline",
                        name
                    );
                    &self.default_pipeline
                })
                .clone()
        } else {
            debug!("No pipeline header found, using default pipeline");
            Arc::clone(&self.default_pipeline)
        };

        Box::pin(async move {
            let router = Arc::try_unwrap(router).unwrap_or_else(|arc_router| (*arc_router).clone());

            match router.oneshot(request).await {
                Ok(response) => Ok(response),
                Err(_) => {
                    // Create a 500 error response
                    let response = axum::response::Response::builder()
                        .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .expect("Failed to build error response");
                    Ok(response)
                }
            }
        })
    }
}
