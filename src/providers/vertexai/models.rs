use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::tool_calls::{ChatMessageToolCall, FunctionCall};
use crate::models::tool_choice::{SimpleToolChoice, ToolChoice};
use crate::models::usage::Usage;
use crate::models::streaming::{ChatCompletionChunk, Choice, ChoiceDelta};

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiChatRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<GeminiToolChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiTool {
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiFunctionDeclaration {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeminiToolChoice {
    None,
    Auto,
    Function(GeminiFunctionChoice),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiFunctionChoice {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiContent {
    pub role: String,
    pub parts: Vec<ContentPart>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentPart {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SafetySetting {
    pub category: String,
    pub threshold: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiChatResponse {
    pub candidates: Vec<GeminiCandidate>,
    pub usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiCandidate {
    pub content: GeminiContent,
    pub finish_reason: Option<String>,
    pub safety_ratings: Option<Vec<SafetyRating>>,
    pub tool_calls: Option<Vec<GeminiToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiToolCall {
    pub function: GeminiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageMetadata {
    pub prompt_token_count: i32,
    pub candidates_token_count: i32,
    pub total_token_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct VertexAIStreamChunk {
    pub candidates: Vec<GeminiCandidate>,
    pub usage_metadata: Option<UsageMetadata>,
}

impl From<ChatCompletionRequest> for GeminiChatRequest {
    fn from(req: ChatCompletionRequest) -> Self {
        let contents = req
            .messages
            .into_iter()
            .map(|msg| GeminiContent {
                role: match msg.role.as_str() {
                    "assistant" => "model".to_string(),
                    role => role.to_string(),
                },
                parts: vec![ContentPart {
                    text: match msg.content {
                        Some(content) => match content {
                            ChatMessageContent::String(text) => text,
                            ChatMessageContent::Array(parts) => parts
                                .into_iter()
                                .map(|p| p.text)
                                .collect::<Vec<_>>()
                                .join(" "),
                        },
                        None => String::new(),
                    },
                }],
            })
            .collect();

        let generation_config = Some(GenerationConfig {
            temperature: req.temperature,
            top_p: req.top_p,
            top_k: None,
            max_output_tokens: req.max_tokens,
            stop_sequences: req.stop,
        });

        let tools = req.tools.map(|tools| {
            vec![GeminiTool {
                function_declarations: tools
                    .into_iter()
                    .map(|tool| GeminiFunctionDeclaration {
                        name: tool.function.name,
                        description: tool.function.description,
                        parameters: serde_json::to_value(tool.function.parameters)
                            .unwrap_or_default(),
                    })
                    .collect(),
            }]
        });

        let tool_choice = req.tool_choice.map(|choice| match choice {
            ToolChoice::Simple(SimpleToolChoice::None) => GeminiToolChoice::None,
            ToolChoice::Simple(SimpleToolChoice::Auto) => GeminiToolChoice::Auto,
            ToolChoice::Named(named) => GeminiToolChoice::Function(GeminiFunctionChoice {
                name: named.function.name,
            }),
            _ => GeminiToolChoice::None,
        });

        Self {
            contents,
            generation_config,
            safety_settings: None,
            tools,
            tool_choice,
        }
    }
}

impl GeminiChatResponse {
    pub fn to_openai(self, model: String) -> ChatCompletion {
        let choices = self
            .candidates
            .into_iter()
            .enumerate()
            .map(|(i, candidate)| ChatCompletionChoice {
                index: i as u32,
                message: ChatCompletionMessage {
                    role: "assistant".to_string(),
                    content: Some(ChatMessageContent::String(
                        candidate
                            .content
                            .parts
                            .into_iter()
                            .map(|p| p.text)
                            .collect::<Vec<_>>()
                            .join(""),
                    )),
                    name: None,
                    tool_calls: candidate.tool_calls.map(|calls| {
                        calls
                            .into_iter()
                            .map(|call| ChatMessageToolCall {
                                id: format!("call_{}", uuid::Uuid::new_v4()),
                                r#type: "function".to_string(),
                                function: FunctionCall {
                                    name: call.function.name,
                                    arguments: call.function.arguments,
                                },
                            })
                            .collect()
                    }),
                },
                finish_reason: candidate.finish_reason,
                logprobs: None,
            })
            .collect();

        let usage = self.usage_metadata.map_or(
            Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            },
            |m| Usage {
                prompt_tokens: m.prompt_token_count as u32,
                completion_tokens: m.candidates_token_count as u32,
                total_tokens: m.total_token_count as u32,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            },
        );

        ChatCompletion {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: Some("chat.completion".to_string()),
            created: Some(chrono::Utc::now().timestamp() as u64),
            model,
            choices,
            usage,
            system_fingerprint: None,
        }
    }
}

impl From<VertexAIStreamChunk> for ChatCompletionChunk {
    fn from(chunk: VertexAIStreamChunk) -> Self {
        let first_candidate = chunk.candidates.first();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            service_tier: None,
            system_fingerprint: None,
            created: chrono::Utc::now().timestamp() as i64,
            model: String::new(),
            choices: vec![Choice {
                index: 0,
                logprobs: None,
                delta: ChoiceDelta {
                    role: None,
                    content: first_candidate
                        .and_then(|c| c.content.parts.first())
                        .map(|p| p.text.clone()),
                    tool_calls: first_candidate
                        .and_then(|c| c.tool_calls.clone())
                        .map(|calls| {
                            calls
                                .into_iter()
                                .map(|call| ChatMessageToolCall {
                                    id: format!("call_{}", uuid::Uuid::new_v4()),
                                    r#type: "function".to_string(),
                                    function: FunctionCall {
                                        name: call.function.name,
                                        arguments: call.function.arguments,
                                    },
                                })
                                .collect()
                        }),
                },
                finish_reason: first_candidate.and_then(|c| c.finish_reason.clone()),
            }],
            usage: None,
        }
    }
} 