use axum::async_trait;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use super::provider::Provider;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionRequest, ChatCompletionResponse,
    ChatMessageContentPart,
};
use crate::models::common::Usage;
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use reqwest::Client;

pub struct AnthropicProvider {
    api_key: String,
    config: ProviderConfig,
    http_client: Client,
}

#[derive(Deserialize, Serialize, Clone)]
struct AnthropicContent {
    pub text: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[derive(Deserialize, Serialize, Clone)]
struct AnthropicChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub content: Vec<AnthropicContent>,
    pub usage: AnthropicUsage,
}

#[derive(Deserialize, Serialize, Clone)]
struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            config: config.clone(),
            http_client: Client::new(),
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "anthropic".to_string()
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let response = self
            .http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&payload)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let status = response.status();
        if status.is_success() {
            let anthropic_response: AnthropicChatCompletionResponse = response
                .json()
                .await
                .expect("Failed to parse Anthropic response");

            Ok(ChatCompletionResponse {
                id: anthropic_response.id,
                object: None,
                created: None,
                model: anthropic_response.model,
                choices: vec![ChatCompletionChoice {
                    index: 0,
                    message: ChatCompletionMessage {
                        name: None,
                        role: "assistant".to_string(),
                        content: crate::models::chat::ChatMessageContent::Array(
                            anthropic_response
                                .content
                                .into_iter()
                                .map(|content| ChatMessageContentPart {
                                    r#type: content.r#type,
                                    text: content.text,
                                })
                                .collect(),
                        ),
                    },
                    finish_reason: Some("stop".to_string()),
                    logprobs: None,
                }],
                usage: Usage {
                    prompt_tokens: anthropic_response.usage.input_tokens,
                    completion_tokens: anthropic_response.usage.output_tokens,
                    total_tokens: anthropic_response.usage.input_tokens
                        + anthropic_response.usage.output_tokens,
                },
            })
        } else {
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        _payload: CompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        unimplemented!()
    }

    async fn embeddings(
        &self,
        _payload: EmbeddingsRequest,
        _model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        unimplemented!()
    }
}
