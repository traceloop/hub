use crate::{pipelines::pipeline::create_pipeline, state::AppState};
use axum::{extract::Request, routing::get, Router};
use std::sync::Arc;
use tower::steer::Steer;

pub fn create_router(state: Arc<AppState>) -> Router {
    let routers = state
        .config
        .pipelines
        .iter()
        .map(|pipeline| create_pipeline(pipeline, &state.model_registry))
        .collect::<Vec<_>>();
    let pipeline_router = Steer::new(routers, |_req: &Request, _services: &[_]| 0);

    Router::new()
        .nest_service("/api/v1", pipeline_router)
        .route("/health", get(|| async { "Working!" }))
        .with_state(state)
}
