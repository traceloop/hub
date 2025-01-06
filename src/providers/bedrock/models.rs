


// I should be able to reuse a lot of the code from antropic's modles


use serde::{Deserialize, Serialize};
use crate::config::constants::default_max_tokens;
use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent, ChatMessageContentPart};
use crate::models::usage::Usage;

#[derive( Serialize, Deserialize , Clone)]
pub struct TitanMessageContent {
    pub text: String,
}

#[derive( Serialize, Deserialize , Clone)]
pub struct TitanMessage {
    pub role: String,
    pub content: Vec<TitanMessageContent>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TitanInferenceConfig {
    pub max_new_tokens: u32,
}

#[derive(Serialize, Deserialize , Clone)]
pub struct TitanChatCompletionRequest {
    #[serde(rename = "inferenceConfig")]
    pub inference_config: TitanInferenceConfig,
    pub messages: Vec<TitanMessage>,
}

#[derive(Deserialize, Serialize)]
pub struct TitanChatCompletionResponse {
    pub output: TitanOutput,
    #[serde(rename = "stopReason")]
    pub stop_reason: String,
    pub usage: TitanUsage,
}


#[derive(Deserialize, Serialize)]
pub struct TitanOutput {
    pub message: TitanMessage,
}

#[derive(Deserialize, Serialize)]
pub struct TitanUsage {
    #[serde(rename = "inputTokens")]
    pub input_tokens: u32,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u32,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u32,
}


impl From<ChatCompletionRequest> for TitanChatCompletionRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        let messages = request.messages.into_iter().map(|msg| {
            let content_text = match msg.content {
                Some(ChatMessageContent::String(text)) => text,
                Some(ChatMessageContent::Array(parts)) => parts
                    .into_iter()
                    .filter(|part| part.r#type == "text")
                    .map(|part| part.text)
                    .collect::<Vec<String>>()
                    .join(" "),
                None => String::new(),
            };

            TitanMessage {
                role: msg.role,
                content: vec![TitanMessageContent {
                    text: content_text,
                }],
            }
        }).collect();

        TitanChatCompletionRequest {
            inference_config: TitanInferenceConfig {
                max_new_tokens: request.max_tokens.unwrap_or(default_max_tokens()),
            },
            messages,
        }
    }
}

impl From<TitanChatCompletionResponse> for ChatCompletion {
    fn from(response: TitanChatCompletionResponse) -> Self {
        let message = ChatCompletionMessage {
            role: response.output.message.role,
            content: Some(ChatMessageContent::String(
                response.output.message.content
                    .into_iter()
                    .map(|c| c.text)
                    .collect::<Vec<String>>()
                    .join(" ")
            )),
            name: None,
            tool_calls: None,
        };

        ChatCompletion {
            id: uuid::Uuid::new_v4().to_string(), // _response.id is private in aws sdk , can't access
            object: None,
            created: None,
            model: "".to_string(),
            choices: vec![ChatCompletionChoice {
                index: 0,
                message,
                finish_reason: Some(response.stop_reason),
                logprobs: None,
            }],
            usage: Usage {
                prompt_tokens: response.usage.input_tokens,
                completion_tokens: response.usage.output_tokens,
                total_tokens: response.usage.total_tokens,
                completion_tokens_details: None,
                prompt_tokens_details: None,
            },
            system_fingerprint: None,
        }
    }
}

