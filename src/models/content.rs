use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum ChatMessageContent {
    String(String),
    Array(Vec<ChatMessageContentPart>),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ChatMessageContentPart {
    #[serde(rename = "type")]
    pub r#type: String,
    pub text: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ChatCompletionMessage {
    pub role: String,
    pub content: ChatMessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
