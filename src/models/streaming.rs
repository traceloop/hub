use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::logprob::ChoiceLogprobs;
use super::tool_calls::ChatMessageToolCall;
use super::usage::Usage;

#[derive(Deserialize, Serialize, Clone, Debug, Default, ToSchema)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub function_call: Option<serde_json::Value>,
    pub tool_calls: Option<Vec<ChatMessageToolCall>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct ChoiceDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatMessageToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct Choice {
    pub delta: ChoiceDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<ChoiceLogprobs>,
}

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub choices: Vec<Choice>,
    pub created: i64,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}
