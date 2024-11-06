use gateway::{config::lib::load_config, routes, state::AppState};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::fmt::Subscriber;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    Subscriber::builder().init();

    info!("Starting the application...");

    let config =
        load_config("config.yaml").map_err(|_| anyhow::anyhow!("Failed to load configuration"))?;
    let state =
        Arc::new(AppState::new(config).map_err(|_| anyhow::anyhow!("Failed to create app state"))?);
    let app = routes::create_router(state);
    let port: String = std::env::var("PORT").unwrap_or("3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    info!("Server is running on port {}", port);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}
