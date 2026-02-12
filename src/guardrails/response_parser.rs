use super::types::{EvaluatorResponse, GuardrailError};
use tracing::debug;

/// Parse the evaluator response body (JSON string) into an EvaluatorResponse.
pub fn parse_evaluator_response(body: &str) -> Result<EvaluatorResponse, GuardrailError> {
    let response = serde_json::from_str::<EvaluatorResponse>(body)
        .map_err(|e| GuardrailError::ParseError(e.to_string()))?;

    // Log for debugging
    debug!(
        pass = response.pass,
        result = %response.result,
        "Parsed evaluator response"
    );

    Ok(response)
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
