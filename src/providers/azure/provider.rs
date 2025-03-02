use axum::async_trait;
use axum::http::StatusCode;
use reqwest_streams::JsonStreamResponse;
use serde_json::Value;

use crate::config::constants::stream_buffer_size_bytes;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse};
use crate::models::streaming::ChatCompletionChunk;
use crate::providers::provider::Provider;
use reqwest::Client;
pub struct AzureProvider {
    config: ProviderConfig,
    http_client: Client,
}

impl AzureProvider {
    fn endpoint(&self) -> String {
        format!(
            "https://{}.openai.azure.com/openai/deployments",
            self.config.params.get("resource_name").unwrap(),
        )
    }

    fn api_version(&self) -> String {
        self.config.params.get("api_version").unwrap().clone()
    }
}

#[async_trait]
impl Provider for AzureProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            config: config.clone(),
            http_client: Client::new(),
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "azure".to_string()
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let deployment = model_config.params.get("deployment").unwrap();
        let api_version = self.api_version();
        let url = format!(
            "{}/{}/chat/completions?api-version={}",
            self.endpoint(),
            deployment,
            api_version
        );

        let response = self
            .http_client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Azure OpenAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            if payload.stream.unwrap_or(false) {
                let stream =
                    response.json_array_stream::<ChatCompletionChunk>(stream_buffer_size_bytes());
                Ok(ChatCompletionResponse::Stream(stream))
            } else {
                response
                    .json()
                    .await
                    .map(ChatCompletionResponse::NonStream)
                    .map_err(|e| {
                        eprintln!("Azure OpenAI API response error: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })
            }
        } else {
            eprintln!(
                "Azure OpenAI API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        payload: CompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        let deployment = model_config.params.get("deployment").unwrap();
        let api_version = self.api_version();
        let url = format!(
            "{}/{}/completions?api-version={}",
            self.endpoint(),
            deployment,
            api_version
        );

        let response = self
            .http_client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Azure OpenAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            response.json().await.map_err(|e| {
                eprintln!("Azure OpenAI API response error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })
        } else {
            eprintln!(
                "Azure OpenAI API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let deployment = model_config.params.get("deployment").unwrap();
        let api_version = self.api_version();

        let url = format!(
            "{}/{}/embeddings?api-version={}",
            self.endpoint(),
            deployment,
            api_version
        );

        let mut azure_payload = match &payload.input {
            EmbeddingsInput::Single(text) => {
                serde_json::json!({
                    "input": text,
                    "model": payload.model
                })
            }
            EmbeddingsInput::Multiple(texts) => {
                serde_json::json!({
                    "input": texts,
                    "model": payload.model
                })
            }
            EmbeddingsInput::SingleTokenIds(token_ids) => {
                // Keep token IDs as is, don't convert to string
                serde_json::json!({
                    "input": token_ids,
                    "model": payload.model
                })
            }
            EmbeddingsInput::MultipleTokenIds(token_ids_list) => {
                // Keep token IDs as is, don't convert to string
                serde_json::json!({
                    "input": token_ids_list,
                    "model": payload.model
                })
            }
        };

        if let Some(user) = &payload.user {
            azure_payload["user"] = Value::String(user.clone());
        }

        if let Some(encoding_format) = &payload.encoding_format {
            azure_payload["encoding_format"] = Value::String(encoding_format.clone());
        }

        let response = self
            .http_client
            .post(&url)
            .header("api-key", &self.config.api_key)
            .json(&azure_payload)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Azure OpenAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            response
                .json()
                .await
                .map_err(|e| {
                        eprintln!("Azure OpenAI Embeddings API response error: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                })
        } else {
            eprintln!(
                "Azure OpenAI Embeddings API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}
