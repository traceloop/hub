use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use sqlx::types::Uuid;

use crate::{
    dto::{CreatePipelineRequestDto, PipelineResponseDto, UpdatePipelineRequestDto},
    errors::ApiError,
    AppState,
};

// --- Pipeline Handlers ---

#[utoipa::path(
    post,
    path = "/api/v1/ee/pipelines",
    request_body = CreatePipelineRequestDto,
    responses(
        (status = 201, description = "Pipeline created successfully", body = PipelineResponseDto),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 409, description = "Conflict - pipeline name already exists", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Pipelines"
)]
#[axum::debug_handler]
async fn create_pipeline_handler(
    State(app_state): State<AppState>,
    Json(payload): Json<CreatePipelineRequestDto>,
) -> Result<(StatusCode, Json<PipelineResponseDto>), ApiError> {
    let result = app_state.pipeline_service.create_pipeline(payload).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

#[utoipa::path(
    get,
    path = "/api/v1/ee/pipelines",
    responses(
        (status = 200, description = "List of pipelines", body = Vec<PipelineResponseDto>),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Pipelines"
)]
#[axum::debug_handler]
async fn list_pipelines_handler(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<PipelineResponseDto>>, ApiError> {
    let result = app_state.pipeline_service.list_pipelines().await?;
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/api/v1/ee/pipelines/{id}",
    params(
        ("id" = Uuid, Path, description = "Pipeline ID")
    ),
    responses(
        (status = 200, description = "Pipeline found", body = PipelineResponseDto),
        (status = 404, description = "Pipeline not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Pipelines"
)]
#[axum::debug_handler]
async fn get_pipeline_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PipelineResponseDto>, ApiError> {
    let result = app_state.pipeline_service.get_pipeline(id).await?;
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/api/v1/ee/pipelines/name/{name}",
    params(
        ("name" = String, Path, description = "Pipeline Name")
    ),
    responses(
        (status = 200, description = "Pipeline found by name", body = PipelineResponseDto),
        (status = 404, description = "Pipeline not found by name", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Pipelines"
)]
#[axum::debug_handler]
async fn get_pipeline_by_name_handler(
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<PipelineResponseDto>, ApiError> {
    let result = app_state
        .pipeline_service
        .get_pipeline_by_name(&name)
        .await?;
    Ok(Json(result))
}

#[utoipa::path(
    put,
    path = "/api/v1/ee/pipelines/{id}",
    request_body = UpdatePipelineRequestDto,
    params(
        ("id" = Uuid, Path, description = "Pipeline ID")
    ),
    responses(
        (status = 200, description = "Pipeline updated successfully", body = PipelineResponseDto),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 404, description = "Pipeline not found", body = ApiError),
        (status = 409, description = "Conflict - pipeline name already exists", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Pipelines"
)]
#[axum::debug_handler]
async fn update_pipeline_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePipelineRequestDto>,
) -> Result<Json<PipelineResponseDto>, ApiError> {
    let result = app_state
        .pipeline_service
        .update_pipeline(id, payload)
        .await?;
    Ok(Json(result))
}

#[utoipa::path(
    delete,
    path = "/api/v1/ee/pipelines/{id}",
    params(
        ("id" = Uuid, Path, description = "Pipeline ID")
    ),
    responses(
        (status = 200, description = "Pipeline deleted successfully"),
        (status = 404, description = "Pipeline not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Pipelines"
)]
#[axum::debug_handler]
async fn delete_pipeline_handler(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, ApiError> {
    // Return Json<()> for successful deletion with no body
    app_state.pipeline_service.delete_pipeline(id).await?;
    Ok(Json(()))
}

// --- Router Definition ---

pub fn pipeline_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            post(create_pipeline_handler).get(list_pipelines_handler),
        )
        .route(
            "/:id",
            get(get_pipeline_handler)
                .put(update_pipeline_handler)
                .delete(delete_pipeline_handler),
        )
        .route("/name/:name", get(get_pipeline_by_name_handler))
}
