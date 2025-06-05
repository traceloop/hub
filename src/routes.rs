use crate::{pipelines::pipeline::create_pipeline, state::AppState};
use axum::{extract::Request, routing::get, Router, http::StatusCode, response::Response};
use axum_prometheus::PrometheusMetricLayerBuilder;
use std::collections::HashMap;
use std::sync::Arc;
use tower::steer::Steer;
pub fn create_router(state: Arc<AppState>) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayerBuilder::new()
        .with_ignore_patterns(&["/metrics", "/health"])
        .with_prefix("traceloop_hub")
        .with_default_metrics()
        .build_pair();

    let mut pipeline_idxs = HashMap::new();
    let mut routers = Vec::new();

    // Sort pipelines to ensure default is first
    let mut sorted_pipelines: Vec<_> = state.current_config().pipelines.clone();
    sorted_pipelines.sort_by_key(|p| p.name != "default"); // "default" will come first since false < true

    for pipeline in sorted_pipelines {
        let name = pipeline.name.clone();
        pipeline_idxs.insert(name, routers.len());
        routers.push(create_pipeline(&pipeline, &state.model_registry()));
    }

    // Always ensure we have at least one router - create a fallback that checks configuration dynamically
    if routers.is_empty() {
        tracing::warn!("No pipelines with routes found. Creating fallback router.");
        let fallback_router = Router::new()
            .fallback(fallback_handler)
            .with_state(state.clone());
        routers.push(fallback_router);
    }

    // Capture the length before moving routers into the closure
    let routers_len = routers.len();

    let pipeline_router = Steer::new(routers, move |req: &Request, _services: &[_]| {
        let index = req.headers()
            .get("x-traceloop-pipeline")
            .and_then(|h| h.to_str().ok())
            .and_then(|name| pipeline_idxs.get(name))
            .copied()
            .unwrap_or(0);
        
        // Ensure the index is within bounds
        if index >= routers_len {
            0
        } else {
            index
        }
    });

    Router::new()
        .nest_service("/api/v1", pipeline_router)
        .route("/health", get(|| async { "Working!" }))
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
        .with_state(state)
}

async fn fallback_handler() -> Result<Response, StatusCode> {
    Err(StatusCode::SERVICE_UNAVAILABLE)
}
