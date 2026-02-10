use super::types::{EvaluatorResponse, GuardrailError};

/// Parse the evaluator response body (JSON string) into an EvaluatorResponse.
pub fn parse_evaluator_response(_body: &str) -> Result<EvaluatorResponse, GuardrailError> {
    todo!("Implement evaluator response parsing")
}

/// Parse an HTTP response from the evaluator, handling non-200 status codes.
pub fn parse_evaluator_http_response(
    _status: u16,
    _body: &str,
) -> Result<EvaluatorResponse, GuardrailError> {
    todo!("Implement HTTP response parsing")
}
