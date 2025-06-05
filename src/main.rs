use hub_lib::{config, routes, state::AppState};
use hub_gateway_core_types::GatewayConfig; // For type hint
use std::sync::Arc;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::{error, info, Level};

// Conditionally import EE crate components
#[cfg(feature = "ee_feature")]
use {
    ee::ee_integration,
    sqlx::PgPool,
    std::time::Duration,
};

const DEFAULT_CONFIG_PATH: &str = "config.yaml";
const DEFAULT_PORT: &str = "3000";
#[cfg(feature = "ee_feature")]
const DEFAULT_DB_POLL_INTERVAL_SECONDS: u64 = 30;

#[cfg(feature = "ee_feature")]
type EeConfigProvider = Arc<ee::services::config_provider_service::ConfigProviderService>;
#[cfg(not(feature = "ee_feature"))]
type EeConfigProvider = ();

async fn get_initial_config_and_services() -> anyhow::Result<(
    GatewayConfig,
    Option<axum::Router>,
    Option<EeConfigProvider>,
)> {
    #[cfg(feature = "ee_feature")]
    {
        info!("EE feature enabled. Attempting to load configuration from database.");
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|e| anyhow::anyhow!("DATABASE_URL not set for EE mode: {}", e))?;
        
        let pool = PgPool::connect(&database_url).await
            .map_err(|e| anyhow::anyhow!("Failed to connect to EE database at {}: {}", database_url, e))?;
        info!("Connected to EE database.");

        // Run EE migrations if CLI arg is present or auto-run is configured
        // sqlx::migrate!("../ee/migrations").run(&pool).await?;
        // info!("EE migrations run successfully.");
        // For now, assume migrations are run separately or by the EE crate itself if it exposes such a utility.

        let ee = ee_integration(pool).await?;
        info!("EE API bundle initialized.");

        match ee.config_provider.fetch_live_config().await {
            Ok(initial_db_config) => {
                info!("Successfully fetched initial configuration from database.");
                if let Err(val_errors) = config::validation::validate_gateway_config(&initial_db_config) {
                    error!("Initial database configuration is invalid: {:?}. Halting.", val_errors);
                    return Err(anyhow::anyhow!("Invalid initial DB config: {:?}", val_errors));
                }
                info!("Initial database configuration validated successfully.");
                Ok((initial_db_config, Some(ee.router.clone()), Some(ee.config_provider.clone())))
            }
            Err(e) => {
                error!("Failed to fetch initial config from DB: {:?}. Halting.", e);
                Err(anyhow::anyhow!("Failed to fetch initial EE config: {}", e))
            }
        }
    }

    #[cfg(not(feature = "ee_feature"))]
    {
        info!("EE feature not enabled. Loading configuration from YAML.");
        let config_path = std::env::var("CONFIG_FILE_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
        info!("Loading configuration from {}", config_path);
        let yaml_config = config::load_config(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to load YAML configuration from {}: {}", config_path, e))?;
        
        if let Err(val_errors) = config::validation::validate_gateway_config(&yaml_config) {
            error!("YAML configuration from {} is invalid: {:?}. Halting.", config_path, val_errors);
            return Err(anyhow::anyhow!("Invalid YAML config: {:?}", val_errors));
        }
        info!("YAML configuration validated successfully.");
        Ok((yaml_config, None, None))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Traceloop Hub Gateway...");

    let (initial_config, _ee_router_opt, _ee_config_provider_opt) = get_initial_config_and_services().await?;

    let app_state = Arc::new(AppState::new(initial_config)
        .map_err(|e| anyhow::anyhow!("Failed to create app state: {}", e))?);

    #[allow(unused_mut)] // main_router is modified when ee_feature is enabled
    let mut main_router = routes::create_router(app_state.clone());

    #[cfg(feature = "ee_feature")]
    {
        if let Some(ee_router) = _ee_router_opt {
            main_router = main_router.nest("/ee/api/v1", ee_router);
            info!("EE Management API router mounted at /ee/api/v1.");
        }

        if let Some(ee_config_provider) = _ee_config_provider_opt {
            // Clone Arcs for the poller task
            let poller_app_state = app_state.clone();
            let poller_config_provider = ee_config_provider.clone();
            
            let poll_interval_seconds = std::env::var("DB_POLL_INTERVAL_SECONDS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(DEFAULT_DB_POLL_INTERVAL_SECONDS);
            let poll_duration = Duration::from_secs(poll_interval_seconds);

            info!("Starting DB configuration poller with interval: {:?}.", poll_duration);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(poll_duration);
                loop {
                    interval.tick().await;
                    info!("Polling database for configuration updates...");
                    match poller_config_provider.fetch_live_config().await {
                        Ok(new_config) => {
                            info!("Successfully fetched updated configuration from database.");
                            info!("New config has {} providers, {} models, {} pipelines", 
                                new_config.providers.len(), 
                                new_config.models.len(), 
                                new_config.pipelines.len());
                            if let Err(val_errors) = config::validation::validate_gateway_config(&new_config) {
                                error!("Updated database configuration is invalid: {:?}. Retaining previous config.", val_errors);
                            } else {
                                info!("Updated database configuration validated. Attempting to apply...");
                                if let Err(update_err) = poller_app_state.try_update_config_and_registries(new_config) {
                                    error!("Failed to apply updated configuration: {:?}", update_err);
                                } else {
                                    info!("Successfully applied updated configuration and rebuilt registries.");
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to fetch updated configuration from DB: {:?}", e);
                        }
                    }
                }
            });
        }
    }

    let app_with_tracing = main_router.layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(()), // Example: Default on_response
    );

    let port_str = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port_str))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind to port {}: {}", port_str, e))?;

    info!("Server is running on port {}", port_str);
    axum::serve(listener, app_with_tracing.into_make_service()).await.unwrap();

    Ok(())
}
