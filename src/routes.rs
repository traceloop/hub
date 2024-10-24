use crate::chat::handlers as chat_handlers;
use crate::{handlers, state::AppState};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    let v1_routes = Router::new()
        .route("/chat/completions", post(chat_handlers::completions))
        .route("/embeddings", post(handlers::embeddings))
        .route("/completions", post(handlers::completions));

    Router::new()
        .route("/health", get(|| async { "Working!" }))
        .nest("/api/v1", v1_routes)
        .with_state(state)
}
