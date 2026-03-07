use crate::models::chat::{ChatCompletion, ChatCompletionRequest};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::content::ChatMessageContent;
use crate::models::embeddings::EmbeddingsRequest;
use tracing::debug;

use super::types::{EvaluatorResponse, GuardrailError};

/// Trait for extracting pre-call guardrail input from a request.
pub trait PromptExtractor {
    fn extract_prompt(&self) -> String;
}

/// Trait for extracting post-call guardrail input from a response.
pub trait CompletionExtractor {
    fn extract_completion(&self) -> String;
}

impl PromptExtractor for ChatCompletionRequest {
    fn extract_prompt(&self) -> String {
        self.messages
            .iter()
            .filter_map(|m| {
                m.content.as_ref().map(|content| match content {
                    ChatMessageContent::String(s) => s.clone(),
                    ChatMessageContent::Array(parts) => parts
                        .iter()
                        .filter(|p| p.r#type == "text")
                        .map(|p| p.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl CompletionExtractor for ChatCompletion {
    fn extract_completion(&self) -> String {
        self.choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .map(|content| match content {
                ChatMessageContent::String(s) => s.clone(),
                ChatMessageContent::Array(parts) => parts
                    .iter()
                    .filter(|p| p.r#type == "text")
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            })
            .unwrap_or_default()
    }
}

impl PromptExtractor for CompletionRequest {
    fn extract_prompt(&self) -> String {
        self.prompt.clone()
    }
}

impl CompletionExtractor for CompletionResponse {
    fn extract_completion(&self) -> String {
        self.choices
            .first()
            .map(|choice| choice.text.clone())
            .unwrap_or_default()
    }
}

impl PromptExtractor for EmbeddingsRequest {
    fn extract_prompt(&self) -> String {
        match &self.input {
            crate::models::embeddings::EmbeddingsInput::Single(s) => s.clone(),
            crate::models::embeddings::EmbeddingsInput::Multiple(v) => v.join("\n"),
            crate::models::embeddings::EmbeddingsInput::SingleTokenIds(_) => {
                "[token IDs]".to_string()
            }
            crate::models::embeddings::EmbeddingsInput::MultipleTokenIds(_) => {
                "[multiple token IDs]".to_string()
            }
        }
    }
}

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
