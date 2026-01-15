use crate::config::constants::default_max_tokens;
use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent, ChatMessageContentPart};
use crate::models::tool_calls::{ChatMessageToolCall, FunctionCall};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct AnthropicChatCompletionRequest {
    pub max_tokens: u32,
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    pub tools: Vec<ToolParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AnthropicChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub content: Vec<ContentBlock>,
    pub usage: Usage,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        input: serde_json::Value,
        name: String,
    },
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Deserialize, Serialize, Clone)]
pub(crate) struct InputSchemaTyped {
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
}

pub(crate) type InputSchema = serde_json::Value;

#[derive(Deserialize, Serialize, Clone)]
pub struct ToolParam {
    pub input_schema: InputSchema,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ToolChoice {
    #[serde(rename = "auto")]
    Auto { disable_parallel_tool_use: bool },
    #[serde(rename = "any")]
    Any { disable_parallel_tool_use: bool },
    #[serde(rename = "tool")]
    Tool {
        name: String,
        disable_parallel_tool_use: bool,
    },
}

impl From<ChatCompletionRequest> for AnthropicChatCompletionRequest {
    fn from(request: ChatCompletionRequest) -> Self {
        let should_include_tools = !matches!(
            request.tool_choice,
            Some(crate::models::tool_choice::ToolChoice::Simple(
                crate::models::tool_choice::SimpleToolChoice::None
            ))
        );

        let mut system = request
            .messages
            .iter()
            .find(|msg| msg.role == "system")
            .and_then(|message| match &message.content {
                Some(ChatMessageContent::String(text)) => Some(text.clone()),
                Some(ChatMessageContent::Array(parts)) => parts
                    .iter()
                    .find(|part| part.r#type == "text")
                    .map(|part| part.text.clone()),
                _ => None,
            });

        // Add reasoning prompt if reasoning is requested
        if let Some(reasoning_config) = &request.reasoning {
            if let Some(thinking_prompt) = reasoning_config.to_thinking_prompt() {
                system = Some(match system {
                    Some(existing) => format!("{existing}\n\n{thinking_prompt}"),
                    None => thinking_prompt,
                });
            }
        }

        let messages: Vec<ChatCompletionMessage> = request
            .messages
            .into_iter()
            .filter(|msg| msg.role != "system")
            .collect();

        let max_tokens = match request.max_completion_tokens {
            Some(val) if val > 0 => val,
            _ => request.max_tokens.unwrap_or_else(default_max_tokens),
        };

        AnthropicChatCompletionRequest {
            max_tokens,
            model: request.model,
            messages,
            temperature: request.temperature,
            top_p: request.top_p,
            stream: request.stream,
            system,
            tool_choice: request.tool_choice.map(|choice| match choice {
                crate::models::tool_choice::ToolChoice::Simple(simple) => match simple {
                    crate::models::tool_choice::SimpleToolChoice::None
                    | crate::models::tool_choice::SimpleToolChoice::Auto => ToolChoice::Auto {
                        disable_parallel_tool_use: request.parallel_tool_calls.unwrap_or(false),
                    },
                    crate::models::tool_choice::SimpleToolChoice::Required => ToolChoice::Any {
                        disable_parallel_tool_use: request.parallel_tool_calls.unwrap_or(false),
                    },
                },
                crate::models::tool_choice::ToolChoice::Named(named) => ToolChoice::Tool {
                    name: named.function.name,
                    disable_parallel_tool_use: request.parallel_tool_calls.unwrap_or(false),
                },
            }),
            tools: if should_include_tools {
                request
                    .tools
                    .unwrap_or_default()
                    .into_iter()
                    .map(|tool| ToolParam {
                        name: tool.function.name,
                        description: tool.function.description,
                        input_schema: serde_json::to_value(tool.function.parameters)
                            .unwrap_or_default(),
                    })
                    .collect()
            } else {
                Vec::new()
            },
        }
    }
}

impl From<Vec<ContentBlock>> for ChatCompletionMessage {
    fn from(blocks: Vec<ContentBlock>) -> Self {
        let mut text_content = Vec::<ChatMessageContentPart>::new();
        let mut tool_calls = Vec::<ChatMessageToolCall>::new();

        for block in blocks {
            match block {
                ContentBlock::Text { text } => {
                    text_content.push(ChatMessageContentPart {
                        r#type: "text".to_string(),
                        text,
                    });
                }
                ContentBlock::ToolUse { name, input, id } => {
                    tool_calls.push(ChatMessageToolCall {
                        id,
                        function: FunctionCall {
                            name,
                            arguments: input.to_string(),
                        },
                        r#type: "function".to_string(),
                    });
                }
            }
        }

        ChatCompletionMessage {
            role: "assistant".to_string(),
            content: Some(ChatMessageContent::Array(text_content)),
            name: None,
            refusal: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
            tool_call_id: None,
        }
    }
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
                message: response.content.into(),
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: crate::models::usage::Usage {
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
