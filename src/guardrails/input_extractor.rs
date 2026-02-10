use crate::models::chat::{ChatCompletion, ChatCompletionRequest};

/// Extract text from the request for pre_call guardrails.
/// Returns the content of the last user message.
pub fn extract_pre_call_input(_request: &ChatCompletionRequest) -> String {
    todo!("Implement pre_call input extraction")
}

/// Extract text from a non-streaming ChatCompletion for post_call guardrails.
/// Returns the content of the first assistant choice.
pub fn extract_post_call_input_from_completion(_completion: &ChatCompletion) -> String {
    todo!("Implement post_call input extraction from completion")
}
