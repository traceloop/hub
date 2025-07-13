use crate::state::AppState;
use axum::{
    Json, Router, body::Body, extract::Request, http::StatusCode, response::Response, routing::get,
    routing::post,
};
use axum_prometheus::PrometheusMetricLayerBuilder;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Service, ServiceExt};
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
        // Add OpenAPI documentation endpoints
        .route(
            "/api-docs/openapi.json",
            get(|| async { Json(crate::openapi::get_openapi_spec()) }),
        )
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
    // With the simplified approach, we always have a current router available
    debug!("Using current pipeline router");
    state.get_current_router()
}

// Removed build_pipeline_router_from_config - now handled in AppState::build_router_for_config

/// Creates a router that explicitly handles API endpoints when no configuration is available
pub fn create_no_config_router() -> Router {
    debug!("Creating no-config fallback router");
    Router::new()
        .route("/chat/completions", post(no_config_handler))
        .route("/completions", post(no_config_handler))
        .route("/embeddings", post(no_config_handler))
        .fallback(no_config_handler)
}

/// Handler that returns 404 when no configuration is available
async fn no_config_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    warn!("No configuration available - returning 404 Not Found");
    Err(StatusCode::NOT_FOUND)
}
