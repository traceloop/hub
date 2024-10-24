use gateway::{routes, state::AppState};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::fmt::Subscriber;

#[tokio::main]
async fn main() {
    Subscriber::builder().init();

    info!("Starting the application...");

    let state = Arc::new(AppState::new());
    let app = routes::create_router(state);
    let port: String = std::env::var("PORT").unwrap_or("3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    info!("Server is running on port {}", port);

    axum::serve(listener, app).await.unwrap();
}
