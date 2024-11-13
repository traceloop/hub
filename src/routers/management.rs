use axum::{
    extract::State, http::StatusCode, routing::{delete, get, post, put}, Json, Router
};
use sqlx::PgPool;

use crate::config::models::Provider;

pub fn management_router(pool: PgPool) -> Router {
    Router::new()
        .route("/providers", get(list_providers))
        .route("/providers", post(create_provider))
        .route("/providers/:name", get(get_provider))
        .route("/providers/:name", put(update_provider))
        .route("/providers/:name", delete(delete_provider))
        .route("/models", get(list_models))
        .route("/models", post(create_model))
        .route("/models/:name", get(get_model))
        .route("/models/:name", put(update_model))
        .route("/models/:name", delete(delete_model))
        .route("/pipelines", get(list_pipelines))
        .route("/pipelines", post(create_pipeline))
        .route("/pipelines/:name", get(get_pipeline))
        .route("/pipelines/:name", put(update_pipeline))
        .route("/pipelines/:name", delete(delete_pipeline))
        .with_state(pool)
}

// Provider handlers
async fn list_providers(
    State(pool): State<PgPool>
) -> Result<Json<Vec<Provider>>, StatusCode> {
    let providers = sqlx::query_as!(
        Provider,
        "SELECT name, provider_type, config FROM providers"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(providers))
}

async fn create_provider(
    State(pool): State<PgPool>,
    Json(provider): Json<Provider>,
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "INSERT INTO providers (name, provider_type, config) VALUES ($1, $2, $3)",
        provider.name,
        provider.provider_type,
        provider.config
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

