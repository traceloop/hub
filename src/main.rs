use hub_lib::types::GatewayConfig;
use hub_lib::{config, routes, state::AppState};
use std::sync::Arc;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::{error, info, Level};

// Always import database components - mode detection happens at runtime
use {hub_lib::management::db_based_config_integration, sqlx::PgPool, std::time::Duration};

#[allow(dead_code)]
const DEFAULT_CONFIG_PATH: &str = "config.yaml";
const DEFAULT_PORT: &str = "3000";
const DEFAULT_DB_POLL_INTERVAL_SECONDS: u64 = 30;

#[derive(Debug, Clone)]
pub enum ConfigMode {
    Yaml { path: String },
    Database { pool: PgPool },
}

type ConfigProvider =
    Arc<hub_lib::management::services::config_provider_service::ConfigProviderService>;

async fn determine_config_mode() -> anyhow::Result<ConfigMode> {
    // Check HUB_MODE environment variable first
    match std::env::var("HUB_MODE").as_deref() {
        Ok("database") => {
            info!("HUB_MODE=database detected. Initializing database mode.");
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|e| anyhow::anyhow!("DATABASE_URL not set for database mode: {}", e))?;

            let pool = PgPool::connect(&database_url).await.map_err(|e| {
                anyhow::anyhow!("Failed to connect to database at {}: {}", database_url, e)
            })?;
            info!("Connected to database successfully.");
            Ok(ConfigMode::Database { pool })
        }
        Ok("yaml") => {
            info!("HUB_MODE=yaml detected. Using YAML configuration mode.");
            let config_path = std::env::var("CONFIG_FILE_PATH")
                .unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
            Ok(ConfigMode::Yaml { path: config_path })
        }
        Ok(invalid_mode) => {
            error!(
                "Invalid HUB_MODE '{}'. Valid options: 'yaml', 'database'",
                invalid_mode
            );
            Err(anyhow::anyhow!("Invalid HUB_MODE: {}", invalid_mode))
        }
        Err(_) => {
            // HUB_MODE not set, fallback to yaml mode
            info!("HUB_MODE not set. Defaulting to YAML configuration mode.");
            let config_path = std::env::var("CONFIG_FILE_PATH")
                .unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
            Ok(ConfigMode::Yaml { path: config_path })
        }
    }
}

async fn get_initial_config_and_services(
    mode: ConfigMode,
) -> anyhow::Result<(GatewayConfig, Option<axum::Router>, Option<ConfigProvider>)> {
    match mode {
        ConfigMode::Database { pool } => {
            info!("Initializing database-based configuration.");

            let db_integration = db_based_config_integration(pool).await?;
            info!("Database integration initialized successfully.");

            match db_integration.config_provider.fetch_live_config().await {
                Ok(initial_db_config) => {
                    info!("Successfully fetched initial configuration from database.");
                    if let Err(val_errors) =
                        config::validation::validate_gateway_config(&initial_db_config)
                    {
                        error!(
                            "Initial database configuration is invalid: {:?}. Halting.",
                            val_errors
                        );
                        return Err(anyhow::anyhow!(
                            "Invalid initial DB config: {:?}",
                            val_errors
                        ));
                    }
                    info!("Initial database configuration validated successfully.");
                    Ok((
                        initial_db_config,
                        Some(db_integration.router.clone()),
                        Some(db_integration.config_provider.clone()),
                    ))
                }
                Err(e) => {
                    error!("Failed to fetch initial config from DB: {:?}. Halting.", e);
                    Err(anyhow::anyhow!("Failed to fetch initial DB config: {}", e))
                }
            }
        }
        ConfigMode::Yaml { path } => {
            info!("Loading configuration from YAML file: {}", path);
            let yaml_config = config::load_config(&path).map_err(|e| {
                anyhow::anyhow!("Failed to load YAML configuration from {}: {}", path, e)
            })?;

            if let Err(val_errors) = config::validation::validate_gateway_config(&yaml_config) {
                error!(
                    "YAML configuration from {} is invalid: {:?}. Halting.",
                    path, val_errors
                );
                return Err(anyhow::anyhow!("Invalid YAML config: {:?}", val_errors));
            }
            info!("YAML configuration validated successfully.");
            Ok((yaml_config, None, None))
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Traceloop Hub Gateway...");

    let config_mode = determine_config_mode().await?;
    info!("Configuration mode determined: {:?}", config_mode);

    let (initial_config, management_router_opt, config_provider_opt) =
        get_initial_config_and_services(config_mode.clone()).await?;

    let app_state = Arc::new(
        AppState::new(initial_config)
            .map_err(|e| anyhow::anyhow!("Failed to create app state: {}", e))?,
    );

    let mut main_router = routes::create_router(app_state.clone());

    // Add management API routes only in database mode
    if let Some(management_router) = management_router_opt {
        main_router = main_router.nest("/api/v1/management", management_router);
        info!("Management API mounted at /api/v1/management (database mode).");
    } else {
        info!("Management API not available (YAML mode).");
    }

    // Start configuration polling only in database mode
    if let Some(config_provider) = config_provider_opt {
        // Clone Arcs for the poller task
        let poller_app_state = app_state.clone();
        let poller_config_provider = config_provider.clone();

        let poll_interval_seconds = std::env::var("DB_POLL_INTERVAL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_DB_POLL_INTERVAL_SECONDS);
        let poll_duration = Duration::from_secs(poll_interval_seconds);

        info!(
            "Starting database configuration poller with interval: {:?}.",
            poll_duration
        );
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_duration);
            loop {
                interval.tick().await;
                info!("Polling database for configuration updates...");
                match poller_config_provider.fetch_live_config().await {
                    Ok(new_config) => {
                        info!("Successfully fetched updated configuration from database.");
                        info!(
                            "New config has {} providers, {} models, {} pipelines",
                            new_config.providers.len(),
                            new_config.models.len(),
                            new_config.pipelines.len()
                        );
                        if let Err(val_errors) =
                            config::validation::validate_gateway_config(&new_config)
                        {
                            error!("Updated database configuration is invalid: {:?}. Retaining previous config.", val_errors);
                        } else {
                            info!(
                                "Updated database configuration validated. Attempting to apply..."
                            );
                            if let Err(update_err) =
                                poller_app_state.try_update_config_and_registries(new_config)
                            {
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

    let app = main_router.layer(
        TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default().include_headers(true)),
    );

    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let bind_address = format!("0.0.0.0:{}", port);

    info!("Starting server on {}", bind_address);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
