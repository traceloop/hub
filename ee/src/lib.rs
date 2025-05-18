pub mod api;
pub mod services;
pub mod db;
pub mod dto;
pub mod errors;

use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

// Repositories - some services might need direct Arc<Repo> access
use crate::db::repositories::{
    // ProviderRepository is created internally by ProviderService via pool
    model_definition_repository::ModelDefinitionRepository, // Needed by PipelineService
    pipeline_repository::PipelineRepository, // Needed by PipelineService
};

// Services
use crate::services::{
    provider_service::ProviderService,
    model_definition_service::ModelDefinitionService,
    pipeline_service::PipelineService,
};

/// Shared application state.
#[derive(Clone, axum::extract::FromRef)]
pub struct AppState {
    pub db_pool: PgPool, // Keep for services that might need direct pool or for test setups
    pub provider_service: Arc<ProviderService>,
    pub model_definition_service: Arc<ModelDefinitionService>,
    pub pipeline_service: Arc<PipelineService>,
}

/// Initializes and returns the Axum router for the EE Management API.
///
/// The router will be nested under a path like "/ee/api/v1" in the main application.
pub fn ee_api_router(pool: PgPool) -> Router { 
    // Repositories that are direct dependencies for some services
    let model_definition_repo_for_pipeline_service = Arc::new(ModelDefinitionRepository::new(pool.clone()));
    let pipeline_repo_for_pipeline_service = Arc::new(PipelineRepository::new(pool.clone()));

    // Initialize services
    // ProviderService and ModelDefinitionService create their own repo instances internally using the pool.
    let provider_service = Arc::new(ProviderService::new(pool.clone()));
    let model_definition_service = Arc::new(ModelDefinitionService::new(pool.clone()));
    // PipelineService takes pre-initialized Arc<Repository> instances.
    let pipeline_service = Arc::new(PipelineService::new(
        pipeline_repo_for_pipeline_service,
        model_definition_repo_for_pipeline_service,
    ));

    let app_state = AppState {
        db_pool: pool.clone(), // Store the pool in AppState as well
        provider_service,
        model_definition_service,
        pipeline_service,
    };

    Router::new()
        .nest("/providers", api::routes::provider_routes::provider_routes()) 
        .nest("/model-definitions", api::routes::model_definition_routes::model_definition_routes())
        .nest("/pipelines", api::routes::pipeline_routes::pipeline_routes()) // These functions return Router<AppState>
        .route("/health", axum::routing::get(|| async {"EE API is healthy"}))
        .with_state(app_state) // Apply AppState to the entire router
}

// Remove or adjust the old `add` function and its test if no longer needed.
// pub fn add(left: u64, right: u64) -> u64 {
//     left + right
// }

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
