use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    DatabaseError(sqlx::Error),
    NotFound(String),
    Conflict(String), // For duplicate entries, etc.
    ValidationError(String), // For DTO validation issues
    // Add other specific error types as needed
    InternalServerError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::DatabaseError(db_err) => {
                // Log the detailed database error to the console for debugging
                eprintln!("Detailed Database Error: {:?}", db_err);
                (StatusCode::INTERNAL_SERVER_ERROR, "A database error occurred".to_string())
            }
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
            _ => ApiError::DatabaseError(err),
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        // More detailed logging for the specific serde error
        let detailed_error_message = format!("Serde JSON Error Kind: {:?}, Message: {}", err.classify(), err);
        eprintln!("Detailed Serde JSON Error before wrapping in ApiError: {}", detailed_error_message);
        ApiError::InternalServerError(format!("JSON processing error: {}", err)) // Keep original response message for client
    }
}
