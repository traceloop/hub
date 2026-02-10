use super::types::{EvaluatorResponse, GuardrailError};

/// Parse the evaluator response body (JSON string) into an EvaluatorResponse.
pub fn parse_evaluator_response(body: &str) -> Result<EvaluatorResponse, GuardrailError> {
    serde_json::from_str::<EvaluatorResponse>(body)
        .map_err(|e| GuardrailError::ParseError(e.to_string()))
}

/// Parse an HTTP response from the evaluator, handling non-200 status codes.
pub fn parse_evaluator_http_response(
    status: u16,
    body: &str,
) -> Result<EvaluatorResponse, GuardrailError> {
    if !(200..300).contains(&status) {
        return Err(GuardrailError::HttpError {
            status,
            body: body.to_string(),
        });
    }
    parse_evaluator_response(body)
}
