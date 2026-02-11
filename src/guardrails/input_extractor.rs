use crate::models::chat::{ChatCompletion, ChatCompletionRequest};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::content::ChatMessageContent;

/// Trait for extracting pre-call guardrail input from a request.
pub trait PreCallInput {
    fn extract_pre_call_input(&self) -> String;
}

impl PreCallInput for ChatCompletionRequest {
    fn extract_pre_call_input(&self) -> String {
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

impl PreCallInput for CompletionRequest {
    fn extract_pre_call_input(&self) -> String {
        self.prompt.clone()
    }
}

/// Extract text from a CompletionResponse for post_call guardrails.
/// Returns the text of the first choice.
pub fn extract_post_call_input_from_completion_response(response: &CompletionResponse) -> String {
    response
        .choices
        .first()
        .map(|choice| choice.text.clone())
        .unwrap_or_default()
}

/// Extract text from a non-streaming ChatCompletion for post_call guardrails.
/// Returns the content of the first assistant choice.
pub fn extract_post_call_input_from_completion(completion: &ChatCompletion) -> String {
    completion
        .choices
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
