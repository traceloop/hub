use hub::{config::lib::load_config, routes, state::AppState};
use std::sync::Arc;
use tracing::info;
use sqlx::PgPool;
use crate::config::{ConfigSource, DatabaseConfig};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();

    info!("Starting Traceloop Hub...");

    let config_path = std::env::args().nth(1).unwrap_or("config.yaml".to_string());
    info!("Loading configuration from {}", config_path);
    let config = load_config(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
    let state = Arc::new(
        AppState::new(config).map_err(|e| anyhow::anyhow!("Failed to create app state: {}", e))?,
    );
    
    let pg = if ConfigSource::from_env() == ConfigSource::Database {
        PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await?
    } else {
        PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await?
    };

    let app = routes::create_router(state, pg);
    let port: String = std::env::var("PORT").unwrap_or("3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    info!("Server is running on port {}", port);
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn load_configuration() -> Result<(Vec<Provider>, Vec<Model>, Vec<Pipeline>), Box<dyn std::error::Error>> {
    match ConfigSource::from_env() {
        ConfigSource::File(path) => {
            let config = std::fs::read_to_string(path)?;
            let config: Config = serde_yaml::from_str(&config)?;
            Ok((config.providers, config.models, config.pipelines))
        }
        ConfigSource::Database(url) => {
            let db_config = DatabaseConfig::new(&url).await?;
            Ok((
                db_config.load_providers().await?,
                db_config.load_models().await?,
                db_config.load_pipelines().await?,
            ))
        }
    }
}
