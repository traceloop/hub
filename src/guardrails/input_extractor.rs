use crate::models::chat::{ChatCompletion, ChatCompletionRequest};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::content::ChatMessageContent;

/// Trait for extracting pre-call guardrail input from a request.
pub trait PromptExtractor {
    fn extract_pompt(&self) -> String;
}

/// Trait for extracting post-call guardrail input from a response.
pub trait CompletionExtractor {
    fn extract_completion(&self) -> String;
}

impl PromptExtractor for ChatCompletionRequest {
    fn extract_pompt(&self) -> String {
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

impl PromptExtractor for CompletionRequest {
    fn extract_pompt(&self) -> String {
        self.prompt.clone()
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

impl CompletionExtractor for CompletionResponse {
    fn extract_completion(&self) -> String {
        self.choices
            .first()
            .map(|choice| choice.text.clone())
            .unwrap_or_default()
    }
}
