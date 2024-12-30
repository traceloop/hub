use crate::config::constants::stream_buffer_size_bytes;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionChoice, CompletionRequest, CompletionResponse};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{Embeddings, EmbeddingsRequest, EmbeddingsResponse};
use crate::models::streaming::ChatCompletionChunk;
use crate::models::usage::Usage;
use crate::models::vertexai::{GeminiChatRequest, GeminiChatResponse};
use crate::providers::provider::Provider;
use axum::async_trait;
use axum::http::StatusCode;
use futures::StreamExt;
use reqwest::Client;
use reqwest_streams::*;
use serde_json::json;

pub struct VertexAIProvider {
    config: ProviderConfig,
    http_client: Client,
    project_id: String,
    location: String,
}

#[async_trait]
impl Provider for VertexAIProvider {
    fn new(config: &ProviderConfig) -> Self {
        let project_id = config
            .params
            .get("project_id")
            .map_or_else(|| "test-project".to_string(), |v| v.to_string());
        let location = config
            .params
            .get("location")
            .map_or_else(|| "us-central1".to_string(), |v| v.to_string());

        Self {
            config: config.clone(),
            http_client: Client::new(),
            project_id,
            location,
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "vertexai".to_string()
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        // Convert OpenAI format to Gemini format
        let gemini_request = GeminiChatRequest::from_openai(payload.clone());

        let endpoint = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent",
            self.location, self.project_id, self.location, payload.model
        );

        let response = self
            .http_client
            .post(&endpoint)
            .bearer_auth(&self.config.api_key)
            .json(&gemini_request)
            .send()
            .await
            .map_err(|e| {
                eprintln!("VertexAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            if payload.stream.unwrap_or(false) {
                let stream = Box::pin(
                    response
                        .json_array_stream::<GeminiChatResponse>(stream_buffer_size_bytes())
                        .map(move |chunk| {
                            chunk
                                .map(|c| ChatCompletionChunk::from_gemini(c, payload.model.clone()))
                                .map_err(|e| {
                                    eprintln!("Error parsing Gemini response: {}", e);
                                    e
                                })
                        }),
                );
                Ok(ChatCompletionResponse::Stream(stream))
            } else {
                let gemini_response = response.json::<GeminiChatResponse>().await.map_err(|e| {
                    eprintln!("VertexAI API response error: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

                Ok(ChatCompletionResponse::NonStream(
                    gemini_response.to_openai(payload.model),
                ))
            }
        } else {
            eprintln!(
                "VertexAI API request error: {}",
                response.text().await.unwrap_or_default()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        payload: CompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        // For Gemini, we'll use the chat endpoint for completions as well
        let chat_request = ChatCompletionRequest {
            model: payload.model,
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(payload.prompt)),
                name: None,
                tool_calls: None,
            }],
            temperature: payload.temperature,
            top_p: payload.top_p,
            n: payload.n,
            stream: payload.stream,
            stop: payload.stop,
            max_tokens: payload.max_tokens,
            presence_penalty: payload.presence_penalty,
            frequency_penalty: payload.frequency_penalty,
            logit_bias: None,
            user: payload.user,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        let chat_response = self.chat_completions(chat_request, model_config).await?;

        // Convert chat response to completion response
        match chat_response {
            ChatCompletionResponse::NonStream(resp) => Ok(CompletionResponse {
                id: resp.id,
                object: "text_completion".to_string(),
                created: resp.created.unwrap_or_default(),
                model: resp.model,
                choices: resp
                    .choices
                    .into_iter()
                    .map(|c| CompletionChoice {
                        text: match c
                            .message
                            .content
                            .unwrap_or(ChatMessageContent::String("".to_string()))
                        {
                            ChatMessageContent::String(s) => s,
                            ChatMessageContent::Array(arr) => arr
                                .into_iter()
                                .map(|p| p.text)
                                .collect::<Vec<_>>()
                                .join(" "),
                        },
                        index: c.index,
                        logprobs: None,
                        finish_reason: c.finish_reason,
                    })
                    .collect(),
                usage: Usage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    prompt_tokens_details: None,
                    completion_tokens_details: None,
                },
            }),
            ChatCompletionResponse::Stream(_) => {
                Err(StatusCode::BAD_REQUEST) // Streaming not supported for completions
            }
        }
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        _model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let endpoint = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:embedContent",
            self.location, self.project_id, self.location, payload.model
        );

        let response = self
            .http_client
            .post(&endpoint)
            .bearer_auth(&self.config.api_key)
            .json(&json!({
                "text": payload.input
            }))
            .send()
            .await
            .map_err(|e| {
                eprintln!("VertexAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        if status.is_success() {
            let gemini_response = response.json::<serde_json::Value>().await.map_err(|e| {
                eprintln!("VertexAI API response error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            // Extract embeddings from Gemini response and convert to OpenAI format
            let embeddings = gemini_response["embeddings"]
                .as_array()
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
                .iter()
                .enumerate()
                .map(|(i, e)| Embeddings {
                    object: "embedding".to_string(),
                    embedding: e["values"]
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect(),
                    index: i,
                })
                .collect();

            Ok(EmbeddingsResponse {
                object: "list".to_string(),
                data: embeddings,
                model: payload.model,
                usage: Usage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    prompt_tokens_details: None,
                    completion_tokens_details: None,
                },
            })
        } else {
            eprintln!(
                "VertexAI API request error: {}",
                response.text().await.unwrap_or_default()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}
