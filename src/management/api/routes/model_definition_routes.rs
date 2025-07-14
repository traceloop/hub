use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use uuid::Uuid;

use crate::management::{
    AppState,
    dto::{CreateModelDefinitionRequest, ModelDefinitionResponse, UpdateModelDefinitionRequest},
    errors::ApiError,
};

pub fn model_definition_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            post(create_model_definition_handler).get(list_model_definitions_handler),
        )
        .route("/key/{key}", get(get_model_definition_by_key_handler))
        .route(
            "/{id}",
            get(get_model_definition_handler)
                .put(update_model_definition_handler)
                .delete(delete_model_definition_handler),
        )
}

#[utoipa::path(
    post,
    path = "/api/v1/management/model-definitions",
    request_body = CreateModelDefinitionRequest,
    responses(
        (status = 200, description = "Model definition created successfully", body = ModelDefinitionResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 409, description = "Conflict - key already exists or provider not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn create_model_definition_handler(
    State(app_state): State<AppState>,
    Json(payload): Json<CreateModelDefinitionRequest>,
) -> Result<(StatusCode, Json<ModelDefinitionResponse>), ApiError> {
    let response = app_state
        .model_definition_service
        .create_model_definition(payload)
        .await?;
    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/management/model-definitions",
    responses(
        (status = 200, description = "List of model definitions", body = Vec<ModelDefinitionResponse>),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn list_model_definitions_handler(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<ModelDefinitionResponse>>, ApiError> {
    let responses = app_state
        .model_definition_service
        .list_model_definitions()
        .await?;
    Ok(Json(responses))
}

#[utoipa::path(
    get,
    path = "/api/v1/management/model-definitions/{id}",
    params(
        ("id" = Uuid, Path, description = "Model Definition ID")
    ),
    responses(
        (status = 200, description = "Model definition found", body = ModelDefinitionResponse),
        (status = 404, description = "Model definition not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn get_model_definition_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ModelDefinitionResponse>, ApiError> {
    let response = app_state
        .model_definition_service
        .get_model_definition(id)
        .await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/management/model-definitions/key/{key}",
    params(
        ("key" = String, Path, description = "Model Definition Key")
    ),
    responses(
        (status = 200, description = "Model definition found by key", body = ModelDefinitionResponse),
        (status = 404, description = "Model definition not found by key", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn get_model_definition_by_key_handler(
    State(app_state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<ModelDefinitionResponse>, ApiError> {
    let response = app_state
        .model_definition_service
        .get_model_definition_by_key(key)
        .await?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/management/model-definitions/{id}",
    request_body = UpdateModelDefinitionRequest,
    params(
        ("id" = Uuid, Path, description = "Model Definition ID")
    ),
    responses(
        (status = 200, description = "Model definition updated successfully", body = ModelDefinitionResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 404, description = "Model definition not found or provider not found", body = ApiError),
        (status = 409, description = "Conflict - key already exists", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn update_model_definition_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateModelDefinitionRequest>,
) -> Result<Json<ModelDefinitionResponse>, ApiError> {
    let response = app_state
        .model_definition_service
        .update_model_definition(id, payload)
        .await?;
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/management/model-definitions/{id}",
    params(
        ("id" = Uuid, Path, description = "Model Definition ID")
    ),
    responses(
        (status = 204, description = "Model definition deleted successfully"),
        (status = 404, description = "Model definition not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn delete_model_definition_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    app_state
        .model_definition_service
        .delete_model_definition(id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
