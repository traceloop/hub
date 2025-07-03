use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use sqlx::types::Uuid;
use std::sync::Arc;

use crate::{
    dto::{CreateModelDefinitionRequest, ModelDefinitionResponse, UpdateModelDefinitionRequest},
    errors::ApiError,
    services::model_definition_service::ModelDefinitionService,
    AppState,
};

pub fn model_definition_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            post(create_model_definition_handler).get(list_model_definitions_handler),
        )
        .route("/key/:key", get(get_model_definition_by_key_handler))
        .route(
            "/:id",
            get(get_model_definition_handler)
                .put(update_model_definition_handler)
                .delete(delete_model_definition_handler),
        )
}

#[utoipa::path(
    post,
    path = "/api/v1/ee/model-definitions",
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
    State(service): State<Arc<ModelDefinitionService>>,
    Json(payload): Json<CreateModelDefinitionRequest>,
) -> Result<(StatusCode, Json<ModelDefinitionResponse>), ApiError> {
    let response = service.create_model_definition(payload).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/ee/model-definitions",
    responses(
        (status = 200, description = "List of model definitions", body = Vec<ModelDefinitionResponse>),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn list_model_definitions_handler(
    State(service): State<Arc<ModelDefinitionService>>,
) -> Result<Json<Vec<ModelDefinitionResponse>>, ApiError> {
    let responses = service.list_model_definitions().await?;
    Ok(Json(responses))
}

#[utoipa::path(
    get,
    path = "/api/v1/ee/model-definitions/{id}",
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
    State(service): State<Arc<ModelDefinitionService>>,
    Path(id_str): Path<String>,
) -> Result<Json<ModelDefinitionResponse>, ApiError> {
    let id = Uuid::parse_str(&id_str)
        .map_err(|_| ApiError::ValidationError(format!("Invalid UUID format: {id_str}")))?;
    let response = service.get_model_definition(id).await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/ee/model-definitions/key/{key}",
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
    State(service): State<Arc<ModelDefinitionService>>,
    Path(key): Path<String>,
) -> Result<Json<ModelDefinitionResponse>, ApiError> {
    let response = service.get_model_definition_by_key(key).await?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/ee/model-definitions/{id}",
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
    State(service): State<Arc<ModelDefinitionService>>,
    Path(id_str): Path<String>,
    Json(payload): Json<UpdateModelDefinitionRequest>,
) -> Result<Json<ModelDefinitionResponse>, ApiError> {
    let id = Uuid::parse_str(&id_str)
        .map_err(|_| ApiError::ValidationError(format!("Invalid UUID format: {id_str}")))?;
    let response = service.update_model_definition(id, payload).await?;
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/ee/model-definitions/{id}",
    params(
        ("id" = Uuid, Path, description = "Model Definition ID")
    ),
    responses(
        (status = 200, description = "Model definition deleted successfully"),
        (status = 404, description = "Model definition not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Model Definitions"
)]
#[axum::debug_handler]
async fn delete_model_definition_handler(
    State(service): State<Arc<ModelDefinitionService>>,
    Path(id_str): Path<String>,
) -> Result<(), ApiError> {
    // Returns 200 OK with no body on success
    let id = Uuid::parse_str(&id_str)
        .map_err(|_| ApiError::ValidationError(format!("Invalid UUID format: {id_str}")))?;
    service.delete_model_definition(id).await?;
    Ok(())
}
