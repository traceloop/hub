use serde::{Deserialize, Serialize};

use super::logprob::ChoiceLogprobs;
use super::tool_calls::{ChatMessageToolCall, FunctionCall};
use super::usage::Usage;
use crate::models::vertexai::GeminiChatResponse;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub function_call: Option<serde_json::Value>,
    pub tool_calls: Option<Vec<ChatMessageToolCall>>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ChoiceDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatMessageToolCall>>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Choice {
    pub delta: ChoiceDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<ChoiceLogprobs>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
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

impl ChatCompletionChunk {
    pub fn from_gemini(response: GeminiChatResponse, model: String) -> Self {
        let first_candidate = response.candidates.first();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            service_tier: None,
            system_fingerprint: None,
            created: chrono::Utc::now().timestamp() as i64,
            model,
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
