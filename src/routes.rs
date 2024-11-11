use crate::{pipelines::{pipeline::create_pipeline, services::model_router::ModelRouterService}, state::AppState};
use axum::{
    extract::Request, http::Response, routing::get, Router
};
use tower_http::services;
use std::{convert::Infallible, sync::Arc};
use tower::{service_fn, steer::Steer};


pub fn create_router(state: Arc<AppState>) -> Router {
    let pipeline_router = Steer::new(
        vec![create_pipeline(state.clone())],
        |_req: &Request, _services: &[_]| {
            0
        },
    );

    Router::new()
        .nest_service("/api/v1", pipeline_router)
        .route("/health", get(|| async { "Working!" }))
        .with_state(state)
}