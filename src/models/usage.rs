use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct CompletionTokensDetails {
    pub accepted_prediction_tokens: Option<u32>,
    pub audio_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub rejected_prediction_tokens: Option<u32>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct PromptTokensDetails {
    pub audio_tokens: Option<u32>,
    pub cached_tokens: Option<u32>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub completion_tokens_details: Option<CompletionTokensDetails>,
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}
