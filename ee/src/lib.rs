pub mod api;
pub mod db;
pub mod dto;
pub mod errors;
pub mod services;
pub mod state;

pub use state::{db_based_config_integration, DbBasedConfigIntegration};

use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

// Repositories - some services might need direct Arc<Repo> access
use crate::db::repositories::{
    // ProviderRepository is created internally by ProviderService via pool
    model_definition_repository::ModelDefinitionRepository, // Needed by PipelineService
    pipeline_repository::PipelineRepository,                // Needed by PipelineService
};

// Services
use crate::services::{
    config_provider_service::ConfigProviderService,
    model_definition_service::ModelDefinitionService, pipeline_service::PipelineService,
    provider_service::ProviderService,
};

/// Shared application state for the DB based config API.
#[derive(Clone, axum::extract::FromRef)]
pub struct AppState {
    pub db_pool: PgPool, // Keep for services that might need direct pool or for test setups
    pub provider_service: Arc<ProviderService>,
    pub model_definition_service: Arc<ModelDefinitionService>,
    pub pipeline_service: Arc<PipelineService>,
    pub config_provider_service: Arc<ConfigProviderService>,
}

/// Initializes and returns the Axum router for the DB based config Management API
/// and the ConfigProviderService for gateway integration.
pub fn management_api_bundle(pool: PgPool) -> (Router, Arc<ConfigProviderService>) {
    // Repositories that are direct dependencies for some services
    let model_definition_repo_for_pipeline_service =
        Arc::new(ModelDefinitionRepository::new(pool.clone()));
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

    // Create ConfigProviderService using the other services
    let config_provider_service = Arc::new(ConfigProviderService::new(
        provider_service.clone(),
        model_definition_service.clone(),
        pipeline_service.clone(),
    ));

    let app_state = AppState {
        db_pool: pool.clone(), // Store the pool in AppState as well
        provider_service,
        model_definition_service,
        pipeline_service,
        config_provider_service: config_provider_service.clone(),
    };

    let router = Router::new()
        .nest(
            "/providers",
            api::routes::provider_routes::provider_routes(),
        )
        .nest(
            "/model-definitions",
            api::routes::model_definition_routes::model_definition_routes(),
        )
        .nest(
            "/pipelines",
            api::routes::pipeline_routes::pipeline_routes(),
        )
        .route(
            "/health",
            axum::routing::get(|| async { "Management API is healthy" }),
        )
        .with_state(app_state);

    (router, config_provider_service) // Return both
}
