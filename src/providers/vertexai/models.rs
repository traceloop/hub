use crate::config::constants::default_max_tokens;
use crate::models::chat::{ChatCompletion, ChatCompletionChoice};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{
    Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse,
};
use crate::models::streaming::{ChatCompletionChunk, Choice, ChoiceDelta};
use crate::models::usage::Usage;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIChatCompletionRequest {
    #[serde(rename = "contents")]
    pub contents: Vec<Content>,
    #[serde(rename = "generation_config")]
    pub generation_config: Option<GenerationConfig>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(rename = "topK")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(rename = "topP")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(rename = "candidateCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<i32>,
    #[serde(rename = "maxOutputTokens")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct Part {
    pub text: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIChatCompletionResponse {
    pub candidates: Vec<GenerateContentResponse>,
    #[serde(rename = "usageMetadata")]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(rename = "modelVersion")]
    pub model_version: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct GenerateContentResponse {
    pub content: Content,
    #[serde(rename = "finishReason")]
    pub finish_reason: String,
    #[serde(rename = "safetyRatings")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
    #[serde(rename = "avgLogprobs")]
    pub avg_logprobs: Option<f32>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: i32,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: i32,
    #[serde(rename = "totalTokenCount")]
    pub total_token_count: i32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct SafetyRating {
    pub category: String,
    pub probability: String,
    #[serde(rename = "probabilityScore")]
    pub probability_score: f32,
    pub severity: String,
    #[serde(rename = "severityScore")]
    pub severity_score: f32,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIStreamChunk {
    pub candidates: Vec<GenerateContentStreamResponse>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct GenerateContentStreamResponse {
    pub content: Option<Content>,
    #[serde(rename = "finishReason")]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIEmbeddingsRequest {
    pub instances: Vec<VertexAIEmbeddingInstance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<EmbeddingParameters>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIEmbeddingInstance {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct EmbeddingParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_truncate: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIEmbeddingsResponse {
    pub predictions: Vec<VertexAIPrediction>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIPrediction {
    pub embeddings: VertexAIEmbeddingValues,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIEmbeddingValues {
    pub values: Vec<f32>,
    pub statistics: VertexAIEmbeddingStatistics,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIEmbeddingStatistics {
    pub truncated: bool,
    pub token_count: u32,
}

impl From<crate::models::chat::ChatCompletionRequest> for VertexAIChatCompletionRequest {
    fn from(request: crate::models::chat::ChatCompletionRequest) -> Self {
        let contents = request
            .messages
            .into_iter()
            .map(|message| {
                let text = match message.content {
                    Some(ChatMessageContent::String(text)) => text,
                    Some(ChatMessageContent::Array(parts)) => parts
                        .into_iter()
                        .map(|part| part.text)
                        .collect::<Vec<_>>()
                        .join(" "),
                    None => String::new(),
                };

                Content {
                    role: match message.role.as_str() {
                        "user" => "user".to_string(),
                        "assistant" => "model".to_string(),
                        _ => "user".to_string(),
                    },
                    parts: vec![Part { text }],
                }
            })
            .collect();

        VertexAIChatCompletionRequest {
            contents,
            generation_config: Some(GenerationConfig {
                temperature: request.temperature,
                top_k: None,
                top_p: request.top_p,
                candidate_count: request.n.map(|n| n as i32),
                max_output_tokens: request.max_tokens.or(Some(default_max_tokens())),
            }),
        }
    }
}

impl From<VertexAIChatCompletionResponse> for ChatCompletion {
    fn from(response: VertexAIChatCompletionResponse) -> Self {
        let choices = response
            .candidates
            .into_iter()
            .enumerate()
            .map(|(index, candidate)| {
                let content = if let Some(part) = candidate.content.parts.first() {
                    ChatMessageContent::String(part.text.clone())
                } else {
                    ChatMessageContent::String(String::new())
                };

                ChatCompletionChoice {
                    index: index as u32,
                    message: ChatCompletionMessage {
                        role: "assistant".to_string(),
                        content: Some(content),
                        name: None,
                        tool_calls: None,
                    },
                    finish_reason: Some(candidate.finish_reason),
                    logprobs: None,
                }
            })
            .collect();

        ChatCompletion {
            id: uuid::Uuid::new_v4().to_string(),
            object: None,
            created: None,
            model: "gemini-pro".to_string(),
            choices,
            usage: crate::models::usage::Usage::default(),
            system_fingerprint: None,
        }
    }
}

impl From<VertexAIStreamChunk> for ChatCompletionChunk {
    fn from(chunk: VertexAIStreamChunk) -> Self {
        ChatCompletionChunk {
            id: uuid::Uuid::new_v4().to_string(),
            choices: chunk
                .candidates
                .into_iter()
                .enumerate()
                .map(|(index, candidate)| Choice {
                    delta: ChoiceDelta {
                        content: candidate
                            .content
                            .and_then(|c| c.parts.first().map(|p| p.text.clone())),
                        role: Some("assistant".to_string()),
                        tool_calls: None,
                    },
                    finish_reason: candidate.finish_reason,
                    index: index as u32,
                    logprobs: None,
                })
                .collect(),
            created: chrono::Utc::now().timestamp(),
            model: "gemini-pro".to_string(),
            system_fingerprint: None,
            service_tier: None,
            usage: None,
        }
    }
}

impl From<EmbeddingsRequest> for VertexAIEmbeddingsRequest {
    fn from(request: EmbeddingsRequest) -> Self {
        let instances = match request.input {
            EmbeddingsInput::Single(text) => vec![VertexAIEmbeddingInstance {
                content: text,
                task_type: None,
            }],
            EmbeddingsInput::Multiple(texts) => texts
                .into_iter()
                .map(|text| VertexAIEmbeddingInstance {
                    content: text,
                    task_type: None,
                })
                .collect(),
        };

        VertexAIEmbeddingsRequest {
            instances,
            parameters: Some(EmbeddingParameters {
                auto_truncate: Some(true),
            }),
        }
    }
}

impl From<VertexAIEmbeddingsResponse> for EmbeddingsResponse {
    fn from(response: VertexAIEmbeddingsResponse) -> Self {
        let token_count = response
            .predictions
            .first()
            .map(|p| p.embeddings.statistics.token_count)
            .unwrap_or(0);

        EmbeddingsResponse {
            object: "list".to_string(),
            data: response
                .predictions
                .into_iter()
                .enumerate()
                .map(|(index, prediction)| Embeddings {
                    object: "embedding".to_string(),
                    embedding: prediction.embeddings.values,
                    index,
                })
                .collect(),
            model: "text-embedding-004".to_string(),
            usage: Usage {
                prompt_tokens: token_count,
                completion_tokens: 0,
                total_tokens: token_count,
                completion_tokens_details: None,
                prompt_tokens_details: None,
            },
        }
    }
}
