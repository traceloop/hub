use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{
    Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse,
};
use crate::models::usage::Usage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub trait BedrockRequestPayload: Serialize {
    fn from_chat_request(request: &ChatCompletionRequest) -> Self;
}

pub trait BedrockResponsePayload: for<'de> Deserialize<'de> {
    fn into_chat_completion(self, model: String) -> ChatCompletion;
}

#[derive(Debug, Serialize)]
pub struct TitanRequest {
    #[serde(rename = "inputText")]
    pub input_text: String,
    #[serde(rename = "textGenerationConfig")]
    pub text_generation_config: TextGenerationConfig,
}

#[derive(Debug, Serialize)]
pub struct TextGenerationConfig {
    #[serde(rename = "maxTokenCount")]
    pub max_token_count: u32,
    pub temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct TitanResponse {
    #[serde(rename = "inputTextTokenCount")]
    pub input_text_token_count: u32,
    pub results: Vec<TitanResult>,
}

#[derive(Debug, Deserialize)]
pub struct TitanResult {
    #[serde(rename = "tokenCount")]
    pub token_count: u32,
    #[serde(rename = "outputText")]
    pub output_text: String,
    #[serde(rename = "completionReason")]
    pub completion_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub prompt: String,
    pub max_tokens_to_sample: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    pub completion: String,
    pub stop_reason: String,
    pub stop: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JurassicRequest {
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub num_results: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct JurassicResponse {
    pub completions: Vec<JurassicCompletion>,
}

#[derive(Debug, Deserialize)]
pub struct JurassicCompletion {
    pub data: JurassicCompletionData,
}

#[derive(Debug, Deserialize)]
pub struct JurassicCompletionData {
    pub text: String,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct TitanEmbeddingsRequest {
    #[serde(rename = "inputText")]
    pub input_text: String,
    pub dimensions: i32,
    pub normalize: bool,
    #[serde(rename = "embeddingTypes")]
    pub embedding_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TitanEmbeddingsResponse {
    pub embedding: Vec<f32>,
}

impl BedrockRequestPayload for TitanRequest {
    fn from_chat_request(request: &ChatCompletionRequest) -> Self {
        let input_text = request
            .messages
            .last()
            .and_then(|msg| msg.content.as_ref())
            .and_then(|content| match content {
                ChatMessageContent::String(text) => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();

        TitanRequest {
            input_text,
            text_generation_config: TextGenerationConfig {
                max_token_count: request.max_tokens.unwrap_or(100),
                temperature: request.temperature.unwrap_or(0.7),
            },
        }
    }
}

impl BedrockRequestPayload for ClaudeRequest {
    fn from_chat_request(request: &ChatCompletionRequest) -> Self {
        let prompt = request
            .messages
            .iter()
            .map(|msg| {
                let content = msg
                    .content
                    .as_ref()
                    .map_or_else(|| "".to_string(), |_| "text".to_string());
                format!("{}: {}", msg.role, content)
            })
            .collect::<Vec<_>>()
            .join("\n");

        ClaudeRequest {
            prompt,
            max_tokens_to_sample: request.max_tokens.unwrap_or(2048),
            temperature: request.temperature,
            top_p: request.top_p,
            stop_sequences: request.stop.clone(),
        }
    }
}

impl BedrockRequestPayload for JurassicRequest {
    fn from_chat_request(request: &ChatCompletionRequest) -> Self {
        let prompt = request
            .messages
            .last()
            .and_then(|msg| msg.content.as_ref())
            .and_then(|content| match content {
                ChatMessageContent::String(text) => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();

        JurassicRequest {
            prompt,
            max_tokens: request.max_tokens.unwrap_or(100),
            temperature: request.temperature.unwrap_or(0.7),
            num_results: 1,
            top_p: request.top_p,
            stop_sequences: request.stop.clone(),
        }
    }
}

impl BedrockResponsePayload for TitanResponse {
    fn into_chat_completion(self, model: String) -> ChatCompletion {
        let result = self.results.first().expect("Expected at least one result");

        ChatCompletion {
            id: Uuid::new_v4().to_string(),
            object: Some("chat.completion".to_string()),
            created: Some(Utc::now().timestamp() as u64),
            model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: "assistant".to_string(),
                    content: Some(ChatMessageContent::String(result.output_text.clone())),
                    name: None,
                    tool_calls: None,
                },
                finish_reason: Some(result.completion_reason.to_lowercase()),
                logprobs: None,
            }],
            usage: Usage {
                prompt_tokens: self.input_text_token_count,
                completion_tokens: result.token_count,
                total_tokens: self.input_text_token_count + result.token_count,
                completion_tokens_details: None,
                prompt_tokens_details: None,
            },
            system_fingerprint: None,
        }
    }
}

impl BedrockResponsePayload for ClaudeResponse {
    fn into_chat_completion(self, model: String) -> ChatCompletion {
        ChatCompletion {
            id: Uuid::new_v4().to_string(),
            object: Some("chat.completion".to_string()),
            created: Some(Utc::now().timestamp() as u64),
            model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: "assistant".to_string(),
                    content: Some(ChatMessageContent::String(self.completion)),
                    name: None,
                    tool_calls: None,
                },
                finish_reason: Some(self.stop_reason),
                logprobs: None,
            }],
            usage: Usage::default(),
            system_fingerprint: None,
        }
    }
}

impl BedrockResponsePayload for JurassicResponse {
    fn into_chat_completion(self, model: String) -> ChatCompletion {
        let completion = self
            .completions
            .first()
            .expect("Expected at least one completion");

        ChatCompletion {
            id: Uuid::new_v4().to_string(),
            object: Some("chat.completion".to_string()),
            created: Some(Utc::now().timestamp() as u64),
            model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: "assistant".to_string(),
                    content: Some(ChatMessageContent::String(completion.data.text.clone())),
                    name: None,
                    tool_calls: None,
                },
                finish_reason: Some(completion.data.finish_reason.clone()),
                logprobs: None,
            }],
            usage: Usage::default(),
            system_fingerprint: None,
        }
    }
}

impl From<EmbeddingsRequest> for TitanEmbeddingsRequest {
    fn from(request: EmbeddingsRequest) -> Self {
        let input_text = match request.input {
            EmbeddingsInput::Single(text) => text,
            EmbeddingsInput::Multiple(texts) => texts.join(" "),
        };

        Self {
            input_text,
            dimensions: 1024,
            normalize: true,
            embedding_types: vec!["float".to_string()],
        }
    }
}

impl TitanEmbeddingsResponse {
    pub fn into_embeddings_response(self, model_name: String) -> EmbeddingsResponse {
        EmbeddingsResponse {
            object: "list".to_string(),
            data: vec![Embeddings {
                object: "embedding".to_string(),
                embedding: self.embedding,
                index: 0,
            }],
            model: model_name,
            usage: Usage::default(),
        }
    }
}
