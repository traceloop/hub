use crate::models::chat::{ChatCompletion, ChatCompletionChoice};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent, ChatMessageContentPart};
use crate::models::usage::Usage;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct AnthropicContent {
    pub text: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct AnthropicChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub content: Vec<AnthropicContent>,
    pub usage: AnthropicUsage,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl From<AnthropicChatCompletionResponse> for ChatCompletion {
    fn from(response: AnthropicChatCompletionResponse) -> Self {
        ChatCompletion {
            id: response.id,
            object: None,
            created: None,
            model: response.model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    name: None,
                    role: "assistant".to_string(),
                    content: ChatMessageContent::Array(
                        response
                            .content
                            .into_iter()
                            .map(|content| ChatMessageContentPart {
                                r#type: content.r#type,
                                text: content.text,
                            })
                            .collect(),
                    ),
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Usage {
                prompt_tokens: response.usage.input_tokens,
                completion_tokens: response.usage.output_tokens,
                total_tokens: response.usage.input_tokens + response.usage.output_tokens,
                completion_tokens_details: None,
                prompt_tokens_details: None,
            },
            system_fingerprint: None,
        }
    }
}
