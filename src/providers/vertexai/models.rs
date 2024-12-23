use crate::config::constants::default_max_tokens;
use crate::models::chat::{ChatCompletion, ChatCompletionChoice};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent}; 
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIChatCompletionRequest {
    pub contents: Vec<VertexAIChatContent>,
    pub model: String,
    pub parameters: VertexAIChatParameters,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIChatParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "role")]
pub(crate) enum VertexAIChatContent {
    #[serde(rename = "user")]
    User(VertexAIChatContentPart),
    #[serde(rename = "model")]
    Model(VertexAIChatContentPart),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIChatContentPart {
    pub parts: Vec<VertexAIChatPart>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "mimeType", content = "data")]
pub(crate) enum VertexAIChatPart {
    #[serde(rename = "text/plain")]
    Text(String),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIChatCompletionResponse {
    pub predictions: Vec<VertexAIChatPrediction>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub(crate) struct VertexAIChatPrediction {
    pub content: String,
}

impl From<crate::models::chat::ChatCompletionRequest> for VertexAIChatCompletionRequest {
    fn from(request: crate::models::chat::ChatCompletionRequest) -> Self {
        let mut contents = Vec::new();
        for message in request.messages {
            let content_parts = match message.content {
                Some(crate::models::content::ChatMessageContent::String(text)) => {
                    vec![VertexAIChatPart::Text(text)]
                }
                Some(crate::models::content::ChatMessageContent::Array(parts)) => parts
                    .into_iter()
                    .filter_map(|part| {
                        if part.r#type == "text" {
                            Some(VertexAIChatPart::Text(part.text))
                        } else {
                            None
                        }
                    })
                    .collect(),
                None => vec![],
            };

            let vertex_content_part = VertexAIChatContentPart {
                parts: content_parts,
            };

            let vertex_content = match message.role.as_str() {
                "user" => VertexAIChatContent::User(vertex_content_part),
                "assistant" => VertexAIChatContent::Model(vertex_content_part),
                _ => continue,
            };
            contents.push(vertex_content);
        }

        VertexAIChatCompletionRequest {
            contents,
            model: request.model,
            parameters: VertexAIChatParameters {
                temperature: request.temperature,
                top_p: request.top_p,
                max_output_tokens: request.max_tokens.or(Some(default_max_tokens())),
            },
        }
    }
}

impl From<VertexAIChatCompletionResponse> for ChatCompletion {
    fn from(response: VertexAIChatCompletionResponse) -> Self {
        let mut choices = Vec::new();
        for (index, prediction) in response.predictions.iter().enumerate() {
            let content = ChatMessageContent::String(prediction.content.clone());
            let message = ChatCompletionMessage {
                role: "assistant".to_string(),
                content: Some(content),
                name: None,
                tool_calls: None,
            };

            choices.push(ChatCompletionChoice {
                index: index as u32,
                message: message,
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            });
        }

        ChatCompletion {
            // Generate a UUID since Vertex AI does not provide an ID.
            id: uuid::Uuid::new_v4().to_string(), 
            object: None,
            created: None,
            model: "".to_string(),
            choices,
            // Vertex AI does not return usage.
            usage: crate::models::usage::Usage::default(),
            system_fingerprint: None,
        }
    }
}
