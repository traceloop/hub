use crate::models::chat::{ChatCompletion, ChatCompletionRequest};
use crate::models::content::ChatMessageContent;

/// Extract text from the request for pre_call guardrails.
/// Returns the content of the last user message.
pub fn extract_pre_call_input(request: &ChatCompletionRequest) -> String {
    request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .and_then(|m| m.content.as_ref())
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
