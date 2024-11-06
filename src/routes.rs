use crate::{handlers, state::AppState};
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderName;
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::compression::CompressionLayer;
use tower_http::propagate_header::PropagateHeaderLayer;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::TraceLayer;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use std::iter::once;
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    let v1_routes = Router::new()
        .route("/chat/completions", post(handlers::chat::completions))
        .route("/embeddings", post(handlers::embeddings::embeddings))
        .route("/completions", post(handlers::completion::completions))
        .layer(SetSensitiveRequestHeadersLayer::new(once(AUTHORIZATION)))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(PropagateHeaderLayer::new(HeaderName::from_static("x-request-id")))
        .layer(ValidateRequestHeaderLayer::accept("application/json"));

    Router::new()
        .route("/health", get(|| async { "Working!" }))
        .nest("/api/v1", v1_routes)
        .with_state(state)
}
