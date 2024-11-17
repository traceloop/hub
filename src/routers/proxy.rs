use crate::pipelines::pipeline::create_pipeline;
use crate::state::AppState;

use axum::extract::Request;
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
use tower::steer::Steer;

pub fn proxy_router(state: Arc<AppState>) -> Router {
    let mut pipeline_idxs = HashMap::new();
    let mut routers = Vec::new();

    // Sort pipelines to ensure default is first
    let mut sorted_pipelines: Vec<_> = state.config.pipelines.clone();
    sorted_pipelines.sort_by_key(|p| p.name != "default"); // "default" will come first since false < true

    for pipeline in sorted_pipelines {
        let name = pipeline.name.clone();
        pipeline_idxs.insert(name, routers.len());
        routers.push(create_pipeline(&pipeline, &state.model_registry));
    }
     
    Steer::new(routers, move |req: &Request, _services: &[_]| {
        *req.headers()
            .get("x-traceloop-pipeline")
            .and_then(|h| h.to_str().ok())
            .and_then(|name| pipeline_idxs.get(name))
            .unwrap_or(&0)
    });
}
