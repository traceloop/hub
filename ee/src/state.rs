use std::sync::Arc;
use axum::Router;
use sqlx::PgPool;
use crate::services::config_provider_service::ConfigProviderService;

pub struct EeIntegration {
    pub router: Router,
    pub config_provider: Arc<ConfigProviderService>,
}

pub async fn ee_integration(pool: PgPool) -> anyhow::Result<EeIntegration> {
    let (router, config_provider) = crate::ee_api_bundle(pool);
    Ok(EeIntegration {
        router,
        config_provider,
    })
} 