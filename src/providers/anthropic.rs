use axum::async_trait;
use axum::http::StatusCode;
use std::sync::Arc;

use super::provider::Provider;
use crate::config::models::Provider as ProviderConfig;
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::common::Usage;
use crate::models::completion::{CompletionChoice, CompletionRequest, CompletionResponse};
use crate::models::embeddings::{
    Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse,
};
use crate::state::AppState;

pub struct AnthropicProvider {
    api_key: String,
    config: ProviderConfig,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            config: config.clone(),
        }
    }

    fn name(&self) -> String {
        self.config.name.clone()
    }

    fn r#type(&self) -> String {
        "anthropic".to_string()
    }

    async fn chat_completions(
        &self,
        state: Arc<AppState>,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let response = state
            .http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("anthropic-version", "2023-06-01")
            .json(&payload)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let status = response.status();
        if status.is_success() {
            response
                .json()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        state: Arc<AppState>,
        payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode> {
        let anthropic_payload = serde_json::json!({
            "model": payload.model,
            "prompt": format!("\n\nHuman: {}\n\nAssistant:", payload.prompt),
            "max_tokens_to_sample": payload.max_tokens.unwrap_or(100),
            "temperature": payload.temperature.unwrap_or(0.7),
            "top_p": payload.top_p.unwrap_or(1.0),
            "stop_sequences": payload.stop.unwrap_or_default(),
        });

        let response = state
            .http_client
            .post("https://api.anthropic.com/v1/complete")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("anthropic-version", "2023-06-01")
            .json(&anthropic_payload)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let status = response.status();
        if !status.is_success() {
            return Err(
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            );
        }

        let anthropic_response: serde_json::Value = response
            .json()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(CompletionResponse {
            id: anthropic_response["completion_id"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            object: "text_completion".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: payload.model,
            choices: vec![CompletionChoice {
                text: anthropic_response["completion"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                index: 0,
                logprobs: None,
                finish_reason: Some("stop".to_string()),
            }],
            usage: Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        })
    }

    async fn embeddings(
        &self,
        state: Arc<AppState>,
        payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let anthropic_payload = match &payload.input {
            EmbeddingsInput::Single(text) => serde_json::json!({
                "model": payload.model,
                "text": text,
            }),
            EmbeddingsInput::Multiple(texts) => serde_json::json!({
                "model": payload.model,
                "text": texts,
            }),
        };

        let response = state
            .http_client
            .post("https://api.anthropic.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("anthropic-version", "2023-06-01")
            .json(&anthropic_payload)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let status = response.status();
        if !status.is_success() {
            return Err(
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            );
        }

        let anthropic_response: serde_json::Value = response
            .json()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let embedding = anthropic_response["embedding"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        Ok(EmbeddingsResponse {
            object: "list".to_string(),
            model: payload.model,
            data: vec![Embeddings {
                object: "embedding".to_string(),
                embedding,
                index: 0,
            }],
            usage: Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        })
    }
}
