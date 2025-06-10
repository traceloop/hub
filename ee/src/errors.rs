use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum ApiError {
    DatabaseError(String), // Changed from sqlx::Error to String for serialization
    NotFound(String),
    Conflict(String),        // For duplicate entries, etc.
    ValidationError(String), // For DTO validation issues
    // Add other specific error types as needed
    InternalServerError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::DatabaseError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            ApiError::Conflict(message) => (StatusCode::CONFLICT, message),
            ApiError::ValidationError(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::InternalServerError(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };

        let body = Json(json!({ "error": error_message }));
        (status, body).into_response()
    }
}

// Convenience for converting sqlx::Error to ApiError
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => ApiError::NotFound("Resource not found".to_string()),
            // Add more specific mappings if needed, e.g., for unique constraint violations
            _ => {
                // Log the detailed database error to the console for debugging
                eprintln!("Detailed Database Error: {:?}", err);
                ApiError::DatabaseError("A database error occurred".to_string())
            }
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        // More detailed logging for the specific serde error
        let detailed_error_message = format!(
            "Serde JSON Error Kind: {:?}, Message: {}",
            err.classify(),
            err
        );
        eprintln!(
            "Detailed Serde JSON Error before wrapping in ApiError: {}",
            detailed_error_message
        );
        ApiError::InternalServerError(format!("JSON processing error: {}", err))
        // Keep original response message for client
    }
}
