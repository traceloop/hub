use crate::pipelines::pipeline::create_pipeline;
use crate::state::AppState;

use axum::extract::Request;
use std::sync::Arc;
use tower::steer::Steer;

pub fn proxy_router(state: Arc<AppState>) -> Steer {
    let routers = state
        .config
        .pipelines
        .iter()
        .map(|pipeline| create_pipeline(pipeline, &state.model_registry))
        .collect::<Vec<_>>();
    
    Steer::new(routers, |_req: &Request, _services: &[_]| 0)
}
