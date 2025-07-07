use crate::management::services::config_provider_service::ConfigProviderService;
use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

pub struct DbBasedConfigIntegration {
    pub router: Router,
    pub config_provider: Arc<ConfigProviderService>,
}

pub async fn db_based_config_integration(pool: PgPool) -> anyhow::Result<DbBasedConfigIntegration> {
    let (router, config_provider) = super::management_api_bundle(pool);
    Ok(DbBasedConfigIntegration {
        router,
        config_provider,
    })
}
