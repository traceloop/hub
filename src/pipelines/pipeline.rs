use std::sync::Arc;

use tower::ServiceBuilder;
use tower_http::{classify::{ServerErrorsAsFailures, SharedClassifier}, trace::{Trace, TraceLayer}};

use crate::state::AppState;

use super::services::model_router::ModelRouterService;


pub fn create_pipeline(state: Arc<AppState>) -> Trace<ModelRouterService, SharedClassifier<ServerErrorsAsFailures>>{
    return ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .service(ModelRouterService::new(state, vec![]));
}


