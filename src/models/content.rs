use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::tool_calls::ChatMessageToolCall;

#[derive(Deserialize, Serialize, Clone, ToSchema)]
#[serde(untagged)]
pub enum ChatMessageContent {
    String(String),
    Array(Vec<ChatMessageContentPart>),
}

#[derive(Deserialize, Serialize, Clone, ToSchema)]
pub struct ChatMessageContentPart {
    #[serde(rename = "type")]
    pub r#type: String,
    pub text: String,
}

#[derive(Deserialize, Serialize, Clone, ToSchema)]
pub struct ChatCompletionMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ChatMessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatMessageToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}
