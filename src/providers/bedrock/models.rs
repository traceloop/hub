


// I should be able to reuse a lot of the code from antropic's modles


use serde::{Deserialize, Serialize};
use crate::config::constants::{default_max_tokens , default_embedding_dimension , default_embedding_normalize};
use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse};
use crate::models::usage::Usage;


/**
 * Titan models
 */

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

#[derive(Debug, Serialize, Deserialize)]
pub struct TitanEmbeddingRequest {
    #[serde(rename = "inputText")]
    pub input_text: String,
    pub dimensions: u32,
    pub normalize: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct  TitanEmbeddingResponse {
    pub embedding: Vec<f32>,
    #[serde(rename = "embeddingsByType")]
    pub embeddings_by_type: EmbeddingsByType,
    #[serde(rename = "inputTextTokenCount")]
    pub input_text_token_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct  EmbeddingsByType {
    pub float: Vec<f32>,
}

impl From<EmbeddingsRequest> for TitanEmbeddingRequest {
    fn from(request: EmbeddingsRequest) -> Self {
        let input_text = match request.input {
            EmbeddingsInput::Single(text) => text,
            EmbeddingsInput::Multiple(texts) => texts.first()
                .map(|s| s.to_string())
                .unwrap_or_default(),
        };

        TitanEmbeddingRequest {
            input_text,
            dimensions: default_embedding_dimension(),
            normalize: default_embedding_normalize(),
        }
    }
}

impl From<TitanEmbeddingResponse> for EmbeddingsResponse {
    fn from(response: TitanEmbeddingResponse) -> Self {
        EmbeddingsResponse {
            object: "list".to_string(),
            data: vec![Embeddings {
                object: "embedding".to_string(),
                embedding: response.embedding,
                index: 0,
            }],
            model: "".to_string(),
            usage: Usage {
                prompt_tokens: response.input_text_token_count,
                completion_tokens: 0,
                total_tokens: response.input_text_token_count,
                completion_tokens_details: None,
                prompt_tokens_details: None,
            },
        }
    }
}



/*
    Ai21 models
*/

#[derive(Debug , Deserialize, Serialize, Clone)]
pub struct Ai21Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug ,Deserialize, Serialize, Clone)]
pub struct Ai21ChatCompletionRequest {
    pub messages: Vec<Ai21Message>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct Ai21ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Ai21Choice>,
    pub model: String,
    pub usage: Ai21Usage,
    pub meta: Ai21Meta,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct Ai21Choice {
    pub finish_reason: String,
    pub index: u32,
    pub message: Ai21Message,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct Ai21Meta {
    #[serde(rename = "requestDurationMillis")]
    pub request_duration_millis: u64,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct Ai21Usage {
    pub completion_tokens: u32,
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

impl From<ChatCompletionRequest> for Ai21ChatCompletionRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        let messages = request.messages.into_iter().map(|msg| {
            let content = match msg.content {
                Some(ChatMessageContent::String(text)) => text,
                Some(ChatMessageContent::Array(parts)) => parts
                    .into_iter()
                    .filter(|part| part.r#type == "text")
                    .map(|part| part.text)
                    .collect::<Vec<String>>()
                    .join(" "),
                None => String::new(),
            };

            Ai21Message {
                role: msg.role,
                content,
            }
        }).collect();

        Ai21ChatCompletionRequest {
            messages,
            max_tokens: request.max_tokens.unwrap_or(default_max_tokens()),
            temperature: request.temperature,
            top_p: request.top_p,
        }
    }
}


impl From<Ai21ChatCompletionResponse> for ChatCompletion {
    fn from(response: Ai21ChatCompletionResponse) -> Self {
        ChatCompletion {
            id: response.id,
            object: None,
            created: None,
            model: response.model,
            choices: response.choices
                .into_iter()
                .map(|choice| ChatCompletionChoice {
                    index: choice.index,
                    message: ChatCompletionMessage {
                        role: choice.message.role,
                        content: Some(ChatMessageContent::String(choice.message.content)),
                        name: None,
                        tool_calls: None,
                    },
                    finish_reason: Some(choice.finish_reason),
                    logprobs: None,
                })
                .collect(),
            usage: Usage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
                completion_tokens_details: None,
                prompt_tokens_details: None,
            },
            system_fingerprint: None,
        }
    }
}