use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::streaming::{ChatCompletionChunk, Choice, ChoiceDelta};
use crate::models::tool_calls::{ChatMessageToolCall, FunctionCall};
use crate::models::tool_choice::{SimpleToolChoice, ToolChoice};
use crate::models::usage::Usage;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSystemInstruction>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiSystemInstruction {
    pub parts: Vec<GeminiSystemPart>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiSystemPart {
    pub text: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(rename = "functionCall", skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
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
    #[serde(rename = "responseMimeType", skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
    #[serde(rename = "responseSchema", skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
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
    pub args: Value,
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

pub fn convert_openai_schema_to_gemini(schema: &Value) -> Result<Value, String> {
    match schema {
        Value::Object(obj) => {
            let mut gemini_obj = serde_json::Map::new();
            
            if let Some(type_val) = obj.get("type") {
                if let Some(type_str) = type_val.as_str() {
                    let gemini_type = match type_str {
                        "string" => "STRING",
                        "number" => "NUMBER", 
                        "integer" => "INTEGER",
                        "boolean" => "BOOLEAN",
                        "array" => "ARRAY",
                        "object" => "OBJECT",
                        _ => return Err(format!("Unsupported type: {}", type_str)),
                    };
                    gemini_obj.insert("type".to_string(), Value::String(gemini_type.to_string()));
                }
            }
            
            if let Some(items) = obj.get("items") {
                let converted_items = convert_openai_schema_to_gemini(items)?;
                gemini_obj.insert("items".to_string(), converted_items);
            }
            
            if let Some(properties) = obj.get("properties") {
                if let Value::Object(props_obj) = properties {
                    let mut converted_props = serde_json::Map::new();
                    let mut property_ordering = Vec::new();
                    
                    // Handle required fields - prioritize them in ordering
                    let required_fields: Vec<String> = if let Some(required) = obj.get("required") {
                        if let Value::Array(req_array) = required {
                            req_array.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };
                    
                    // Add required fields first to property ordering
                    for req_field in &required_fields {
                        if props_obj.contains_key(req_field) {
                            property_ordering.push(Value::String(req_field.clone()));
                        }
                    }
                    
                    // Add remaining fields to property ordering
                    for prop_name in props_obj.keys() {
                        if !required_fields.contains(prop_name) {
                            property_ordering.push(Value::String(prop_name.clone()));
                        }
                    }
                    
                    // Convert all properties (don't add individual required fields)
                    for (prop_name, prop_schema) in props_obj {
                        let converted_prop = convert_openai_schema_to_gemini(prop_schema)?;
                        converted_props.insert(prop_name.clone(), converted_prop);
                    }
                    
                    gemini_obj.insert("properties".to_string(), Value::Object(converted_props));
                    gemini_obj.insert("propertyOrdering".to_string(), Value::Array(property_ordering));
                    
                    // Add required fields as an array at the schema level if there are any
                    if !required_fields.is_empty() {
                        let required_array: Vec<Value> = required_fields.iter()
                            .map(|field| Value::String(field.clone()))
                            .collect();
                        gemini_obj.insert("required".to_string(), Value::Array(required_array));
                    }
                }
            }
            
            // Handle additionalProperties (Gemini doesn't support this directly)
            if obj.contains_key("additionalProperties") {
                // Just ignore additionalProperties as Gemini doesn't support it
            }
            
            if let Some(description) = obj.get("description") {
                gemini_obj.insert("description".to_string(), description.clone());
            }
            
            Ok(Value::Object(gemini_obj))
        },
        _ => Err("Schema must be an object".to_string()),
    }
}

impl From<ChatCompletionRequest> for GeminiChatRequest {
    fn from(req: ChatCompletionRequest) -> Self {
        let system_instruction = req
            .messages
            .iter()
            .find(|msg| msg.role == "system")
            .and_then(|message| match &message.content {
                Some(ChatMessageContent::String(text)) => Some(GeminiSystemInstruction {
                    parts: vec![GeminiSystemPart { text: text.clone() }],
                }),
                Some(ChatMessageContent::Array(parts)) => parts
                    .iter()
                    .find(|part| part.r#type == "text")
                    .map(|part| GeminiSystemInstruction {
                        parts: vec![GeminiSystemPart {
                            text: part.text.clone(),
                        }],
                    }),
                _ => None,
            });

        let contents = req
            .messages
            .into_iter()
            .filter(|msg| msg.role != "system")
            .map(|msg| GeminiContent {
                role: match msg.role.as_str() {
                    "assistant" => "model".to_string(),
                    role => role.to_string(),
                },
                parts: vec![ContentPart {
                    text: match msg.content {
                        Some(content) => match content {
                            ChatMessageContent::String(text) => Some(text),
                            ChatMessageContent::Array(parts) => Some(
                                parts
                                    .into_iter()
                                    .map(|p| p.text)
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            ),
                        },
                        None => None,
                    },
                    function_call: None,
                }],
            })
            .collect();

        let (response_mime_type, response_schema) = if let Some(response_format) = &req.response_format {
            if response_format.r#type == "json_schema" {
                if let Some(json_schema) = &response_format.json_schema {
                    if let Some(schema) = &json_schema.schema {
                        match convert_openai_schema_to_gemini(schema) {
                            Ok(gemini_schema) => (
                                Some("application/json".to_string()),
                                Some(gemini_schema)
                            ),
                            Err(_) => (None, None)
                        }
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else if response_format.r#type == "json_object" {
                // For json_object type, set MIME type but no schema
                (Some("application/json".to_string()), None)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };


        
        let generation_config = Some(GenerationConfig {
            temperature: req.temperature,
            top_p: req.top_p,
            top_k: None,
            max_output_tokens: req.max_tokens,
            stop_sequences: req.stop,
            response_mime_type: response_mime_type.clone(),
            response_schema: response_schema.clone(),
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
            system_instruction,
        }
    }
}

impl GeminiChatResponse {
    pub fn to_openai(self, model: String) -> ChatCompletion {
        self.to_openai_with_structured_output(model, false)
    }
    
    pub fn to_openai_with_structured_output(self, model: String, is_structured_output: bool) -> ChatCompletion {
        let choices = self
            .candidates
            .into_iter()
            .enumerate()
            .map(|(i, candidate)| {
                let mut message_text = String::new();
                let mut tool_calls = Vec::new();

                for part in candidate.content.parts {
                    if let Some(text) = part.text {
                        if is_structured_output {
                            // Check if the text looks like JSON and try to parse it
                            let trimmed_text = text.trim();
                            if (trimmed_text.starts_with('{') && trimmed_text.ends_with('}')) || 
                               (trimmed_text.starts_with('[') && trimmed_text.ends_with(']')) {
                                // Validate that it's proper JSON
                                match serde_json::from_str::<serde_json::Value>(trimmed_text) {
                                    Ok(_) => {
                                        message_text.push_str(trimmed_text);
                                    },
                                    Err(_) => {
                                        message_text.push_str(&text);
                                    }
                                }
                            } else {
                                message_text.push_str(&text);
                            }
                        } else {
                            message_text.push_str(&text);
                        }
                    }
                    if let Some(fc) = part.function_call {
                        tool_calls.push(ChatMessageToolCall {
                            id: format!("call_{}", uuid::Uuid::new_v4()),
                            r#type: "function".to_string(),
                            function: FunctionCall {
                                name: fc.name,
                                arguments: serde_json::to_string(&fc.args)
                                    .unwrap_or_else(|_| "{}".to_string()),
                            },
                        });
                    }
                }

                ChatCompletionChoice {
                    index: i as u32,
                    message: ChatCompletionMessage {
                        role: "assistant".to_string(),
                        content: if message_text.is_empty() {
                            None
                        } else {
                            Some(ChatMessageContent::String(message_text))
                        },
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
                        name: None,
                        refusal: None,
                    },
                    finish_reason: candidate.finish_reason,
                    logprobs: None,
                }
            })
            .collect();

        let usage = self.usage_metadata.map_or_else(
            || Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                completion_tokens_details: None,
                prompt_tokens_details: None,
            },
            |meta| Usage {
                prompt_tokens: meta.prompt_token_count as u32,
                completion_tokens: meta.candidates_token_count as u32,
                total_tokens: meta.total_token_count as u32,
                completion_tokens_details: None,
                prompt_tokens_details: None,
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
            created: chrono::Utc::now().timestamp(),
            model: String::new(),
            choices: vec![Choice {
                index: 0,
                logprobs: None,
                delta: ChoiceDelta {
                    role: None,
                    content: first_candidate
                        .and_then(|c| c.content.parts.first())
                        .map(|p| p.text.clone().unwrap_or_default()),
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
                                        arguments: serde_json::to_string(&call.function.args)
                                            .unwrap_or_else(|_| "{}".to_string()),
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
