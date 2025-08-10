use futures::stream::BoxStream;
use reqwest_streams::error::StreamBodyError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use super::content::ChatCompletionMessage;
use super::logprob::LogProbs;
use super::response_format::ResponseFormat;
use super::streaming::ChatCompletionChunk;
use super::tool_choice::ToolChoice;
use super::tool_definition::ToolDefinition;
use super::usage::Usage;

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>, // "low" | "medium" | "high"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>, // Alternative to effort
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<bool>, // Whether to exclude from response (default: false)
}

impl ReasoningConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.effort.is_some() && self.max_tokens.is_some() {
            tracing::warn!("Both effort and max_tokens specified - prioritizing max_tokens");
        }

        // Only validate effort if max_tokens is not present (since max_tokens takes priority)
        if let Some(effort) = &self.effort {
            if effort.trim().is_empty() {
                return Err("Effort cannot be empty string".to_string());
            } else if self.max_tokens.is_none()
                && !["low", "medium", "high"].contains(&effort.as_str())
            {
                return Err("Invalid effort value. Must be 'low', 'medium', or 'high'".to_string());
            }
        }

        Ok(())
    }

    // For OpenAI/Azure - Direct passthrough (but prioritize max_tokens over effort)
    pub fn to_openai_effort(&self) -> Option<String> {
        if self.max_tokens.is_some() {
            // If max_tokens is specified, don't use effort for OpenAI
            None
        } else {
            // Only return effort if it's not empty
            self.effort
                .as_ref()
                .filter(|e| !e.trim().is_empty())
                .cloned()
        }
    }

    // For Vertex AI (Gemini) - Use max_tokens directly
    pub fn to_gemini_thinking_budget(&self) -> Option<i32> {
        self.max_tokens.map(|tokens| tokens as i32)
    }

    // For Anthropic/Bedrock - Custom prompt generation (prioritize max_tokens over effort)
    pub fn to_thinking_prompt(&self) -> Option<String> {
        if self.max_tokens.is_some() {
            // If max_tokens is specified, use a generic thinking prompt
            Some("Think through this step-by-step with detailed reasoning.".to_string())
        } else {
            match self.effort.as_deref() {
                Some(effort) if !effort.trim().is_empty() => match effort {
                    "high" => {
                        Some("Think through this step-by-step with detailed reasoning.".to_string())
                    }
                    "medium" => Some("Consider this problem thoughtfully.".to_string()),
                    "low" => Some("Think about this briefly.".to_string()),
                    _ => None,
                },
                _ => None,
            }
        }
    }
}

#[derive(Deserialize, Serialize, Clone, ToSchema)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,
}

// Note: ChatCompletionResponse cannot derive ToSchema due to BoxStream
// For OpenAPI documentation, we'll document ChatCompletion directly
pub enum ChatCompletionResponse {
    Stream(BoxStream<'static, Result<ChatCompletionChunk, StreamBodyError>>),
    NonStream(ChatCompletion),
}

#[derive(Deserialize, Serialize, Clone, ToSchema)]
pub struct ChatCompletion {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Usage,
    pub system_fingerprint: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, ToSchema)]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: ChatCompletionMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}
