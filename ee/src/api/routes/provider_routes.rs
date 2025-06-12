use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use sqlx::types::Uuid;

use crate::{
    dto::{CreateProviderRequest, ProviderResponse, UpdateProviderRequest},
    errors::ApiError,
    AppState,
};

/// Creates the Axum router for provider CRUD operations.
pub fn provider_routes() -> Router<AppState> {
    // No longer takes AppState, returns Router<AppState>
    Router::new()
        .route(
            "/",
            post(create_provider_handler).get(list_providers_handler),
        )
        .route(
            "/:id",
            get(get_provider_handler)
                .put(update_provider_handler)
                .delete(delete_provider_handler),
        )
}

#[utoipa::path(
    post,
    path = "/ee/api/v1/providers",
    request_body = CreateProviderRequest,
    responses(
        (status = 201, description = "Provider created successfully", body = ProviderResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 409, description = "Conflict - provider name already exists", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Providers"
)]
#[axum::debug_handler]
async fn create_provider_handler(
    State(app_state): State<AppState>,
    Json(payload): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<ProviderResponse>), ApiError> {
    let service = &app_state.provider_service;
    let provider_response = service.create_provider(payload).await?;
    Ok((StatusCode::CREATED, Json(provider_response)))
}

#[utoipa::path(
    get,
    path = "/ee/api/v1/providers",
    responses(
        (status = 200, description = "List of providers", body = Vec<ProviderResponse>),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Providers"
)]
#[axum::debug_handler]
async fn list_providers_handler(
    State(app_state): State<AppState>,
) -> Result<(StatusCode, Json<Vec<ProviderResponse>>), ApiError> {
    let service = &app_state.provider_service;
    let provider_responses = service.list_providers().await?;
    Ok((StatusCode::OK, Json(provider_responses)))
}

#[utoipa::path(
    get,
    path = "/ee/api/v1/providers/{id}",
    params(
        ("id" = Uuid, Path, description = "Provider ID")
    ),
    responses(
        (status = 200, description = "Provider found", body = ProviderResponse),
        (status = 404, description = "Provider not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Providers"
)]
#[axum::debug_handler]
async fn get_provider_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProviderResponse>, ApiError> {
    let service = &app_state.provider_service;
    let provider_response = service.get_provider(id).await?;
    Ok(Json(provider_response))
}

#[utoipa::path(
    put,
    path = "/ee/api/v1/providers/{id}",
    request_body = UpdateProviderRequest,
    params(
        ("id" = Uuid, Path, description = "Provider ID")
    ),
    responses(
        (status = 200, description = "Provider updated successfully", body = ProviderResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 404, description = "Provider not found", body = ApiError),
        (status = 409, description = "Conflict - provider name already exists", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Providers"
)]
#[axum::debug_handler]
async fn update_provider_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderResponse>, ApiError> {
    let service = &app_state.provider_service;
    let provider_response = service.update_provider(id, payload).await?;
    Ok(Json(provider_response))
}

#[utoipa::path(
    delete,
    path = "/ee/api/v1/providers/{id}",
    params(
        ("id" = Uuid, Path, description = "Provider ID")
    ),
    responses(
        (status = 200, description = "Provider deleted successfully"), // Or 204 No Content
        (status = 404, description = "Provider not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Providers"
)]
#[axum::debug_handler]
async fn delete_provider_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<(), ApiError> {
    let service = &app_state.provider_service;
    service.delete_provider(id).await
}
