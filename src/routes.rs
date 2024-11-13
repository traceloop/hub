use std::sync::Arc;

use axum::{routing::get, Router};
use sqlx::PgPool;

use crate::{routers::{management::management_router, proxy::proxy_router}, state::AppState};

pub fn create_router(state: Arc<AppState>, pg: PgPool) -> Router {
    Router::new()
        .nest_service("/api/v1", proxy_router)
        .with_state(state)
        .nest_service("/management/api/v1", management_router(pg))
        .route("/health", get(|| async { "Working!" }))
}
