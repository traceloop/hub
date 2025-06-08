use crate::{pipelines::pipeline::create_pipeline, state::AppState};
use axum::{
    body::Body, extract::Request, http::StatusCode, response::Response, routing::get,
    routing::post, Json, Router,
};
use axum_prometheus::PrometheusMetricLayerBuilder;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{steer::Steer, Service, ServiceExt};
use tracing::{debug, warn};

pub fn create_router(state: Arc<AppState>) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayerBuilder::new()
        .with_ignore_patterns(&["/metrics", "/health"])
        .with_prefix("traceloop_hub")
        .with_default_metrics()
        .build_pair();

    // Create a dynamic service that forwards to the current pipeline router
    let dynamic_service = DynamicPipelineService::new(state.clone());

    Router::new()
        .nest_service("/api/v1", dynamic_service)
        .route("/health", get(|| async { "Working!" }))
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
        .with_state(state)
}

/// A service that dynamically forwards requests to the current pipeline router
#[derive(Clone)]
pub struct DynamicPipelineService {
    state: Arc<AppState>,
}

impl DynamicPipelineService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl Service<Request<Body>> for DynamicPipelineService {
    type Response = Response;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let state = self.state.clone();

        Box::pin(async move {
            // Get the current dynamic router
            let current_router = create_dynamic_pipeline_router(state);

            // Forward the request to the current router
            match current_router.oneshot(request).await {
                Ok(response) => Ok(response),
                Err(_) => {
                    // Create a 500 error response
                    let response = Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap();
                    Ok(response)
                }
            }
        })
    }
}

/// Creates a dynamic pipeline router that can rebuild itself when configuration changes
pub fn create_dynamic_pipeline_router(state: Arc<AppState>) -> Router {
    // Check if we have a cached router first
    if let Some(cached) = state.get_cached_pipeline_router() {
        debug!("Using cached pipeline router");
        return cached;
    }

    debug!("Building new pipeline router from current configuration");
    let router = build_pipeline_router_from_config(state.clone());

    // Move router into cache, then retrieve from cache to avoid cloning
    state.set_cached_pipeline_router(router);
    // Safe unwrap since we just set it
    state.get_cached_pipeline_router().unwrap()
}

/// Builds the actual pipeline router from the current configuration
/// This is used by create_dynamic_pipeline_router for external calls
fn build_pipeline_router_from_config(state: Arc<AppState>) -> Router {
    let mut pipeline_idxs = HashMap::new();
    let mut routers = Vec::new();

    // Get current configuration
    let current_config = state.current_config();
    let model_registry = state.model_registry();

    debug!(
        "Building router with {} pipelines",
        current_config.pipelines.len()
    );

    // Sort pipelines to ensure default is first
    let mut sorted_pipelines = current_config.pipelines.clone();
    sorted_pipelines.sort_by_key(|p| p.name != "default"); // "default" will come first since false < true

    for pipeline in sorted_pipelines {
        let name = pipeline.name.clone();
        debug!(
            "Adding pipeline '{}' to router at index {}",
            name,
            routers.len()
        );
        pipeline_idxs.insert(name, routers.len());
        routers.push(create_pipeline(&pipeline, &model_registry));
    }

    // Always ensure we have at least one router - create a fallback that checks configuration dynamically
    if routers.is_empty() {
        warn!("No pipelines with routes found. Creating fallback router that returns 503.");
        let fallback_router = create_no_config_router(state.clone());
        routers.push(fallback_router);
        debug!("Fallback router created and added at index 0");
    }

    // Capture the length before moving routers into the closure
    let routers_len = routers.len();
    debug!(
        "Router steering configured with {} total routers",
        routers_len
    );

    let pipeline_router = Steer::new(routers, move |req: &Request, _services: &[_]| {
        let pipeline_header = req
            .headers()
            .get("x-traceloop-pipeline")
            .and_then(|h| h.to_str().ok());

        let index = pipeline_header
            .and_then(|name| pipeline_idxs.get(name))
            .copied()
            .unwrap_or(0);

        // Ensure the index is within bounds
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
    });

    Router::new().nest_service("/", pipeline_router)
}

/// Creates a router that explicitly handles API endpoints when no configuration is available
fn create_no_config_router(state: Arc<AppState>) -> Router {
    debug!("Creating no-config fallback router");
    Router::new()
        .route("/chat/completions", post(no_config_handler))
        .route("/completions", post(no_config_handler))
        .route("/embeddings", post(no_config_handler))
        .fallback(no_config_handler)
        .with_state(state)
}

/// Handler that returns 503 when no configuration is available
async fn no_config_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    warn!("No configuration available - returning 404 Not Found");
    Err(StatusCode::NOT_FOUND)
}
