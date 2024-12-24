use super::models::{VertexAIChatCompletionRequest, VertexAIChatCompletionResponse, VertexAIStreamChunk};
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;
use axum::async_trait;
use axum::http::StatusCode;
use gcp_auth::{CustomServiceAccount, TokenProvider};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::config::constants::stream_buffer_size_bytes;
use reqwest_streams::JsonStreamResponse;
use reqwest_streams::error::{StreamBodyError, StreamBodyKind};
use futures::StreamExt;

pub struct VertexAIProvider {
    config: ProviderConfig,
    http_client: Client,
    token_provider: Arc<Mutex<Option<Arc<dyn TokenProvider>>>>,
}

#[async_trait]
impl Provider for VertexAIProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            config: config.clone(),
            http_client: Client::new(),
            token_provider: Arc::new(Mutex::new(None)),
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
    // Extract necessary information from the request and config
    let model = payload.model.clone();
    let token = self.get_token().await?;
    let request: VertexAIChatCompletionRequest = payload.clone().into();
    
    // Get required configuration parameters
    let project_id = self
        .config
        .params
        .get("project_id")
        .ok_or(StatusCode::BAD_REQUEST)?;
    let location = self
        .config
        .params
        .get("location")
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Choose the appropriate endpoint based on whether streaming is requested
    // VertexAI has different endpoints for streaming vs non-streaming responses
    let endpoint = if payload.stream.unwrap_or(false) {
        "streamGenerateContent"
    } else {
        "generateContent"
    };

    // Construct the full API URL using the provided parameters
    let url = format!(
        "https://{location}-aiplatform.googleapis.com/v1/projects/{project_id}/locations/{location}/publishers/google/models/{model}:{endpoint}",
        location = location,
        project_id = project_id,
        model = model
    );

    // Set up request headers including authentication
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {}", token))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    // Prepare the request body with the message contents and generation configuration
    let request_body = json!({
        "contents": request.contents,
        "generation_config": request.generation_config,
    });

    // Send the request to VertexAI
    let response = self
        .http_client
        .post(url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            eprintln!("VertexAI API request error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Check if the request was successful
    let status = response.status();
    if !status.is_success() {
        // If request failed, extract and log the error message
        let error_text = response.text().await.unwrap_or_default();
        eprintln!("VertexAI API request error: {}", error_text);
        return Err(StatusCode::from_u16(status.as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
    }

    // Handle the response based on whether streaming was requested
    if payload.stream.unwrap_or(false) {
        // For streaming responses, set up a stream processor that:
        // 1. Reads the response as a stream of JSON chunks
        // 2. Maps each chunk to our common format
        // 3. Handles any errors that occur during streaming
        let buffer_size = stream_buffer_size_bytes() * 10;
        let stream = response
            .json_array_stream::<VertexAIStreamChunk>(buffer_size)
            .map(|result| {
                result.map(|chunk| chunk.into())
                    .map_err(|e| StreamBodyError::new(
                        StreamBodyKind::CodecError,
                        Some(Box::new(e)),
                        None
                    ))
            });
        
        Ok(ChatCompletionResponse::Stream(Box::pin(stream)))
    } else {
        // For non-streaming responses, parse the entire response as a single JSON object
        let vertex_response: VertexAIChatCompletionResponse = response.json().await
            .map_err(|e| {
                eprintln!("VertexAI API response error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        Ok(ChatCompletionResponse::NonStream(vertex_response.into()))
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

impl VertexAIProvider {
    async fn get_token(&self) -> Result<String, StatusCode> {
        let provider = {
            let guard = self.token_provider.lock().await;

            match guard.as_ref() {
                Some(p) => p.clone(),
                None => {
                    drop(guard);
                    let new_provider = self.ensure_token_provider().await?;
                    let mut guard = self.token_provider.lock().await;
                    *guard = Some(new_provider.clone());
                    new_provider
                }
            }
        };

        let token = provider
            .token(&["https://www.googleapis.com/auth/cloud-platform"])
            .await
            .map_err(|e| {
                eprintln!("Authentication error: {}", e);
                StatusCode::UNAUTHORIZED
            })?;

        Ok(token.as_str().to_string())
    }

    async fn ensure_token_provider(&self) -> Result<Arc<dyn TokenProvider>, StatusCode> {
        match self.config.params.get("credentials_path") {
            Some(path) => {
                let service_account = CustomServiceAccount::from_file(path).map_err(|e| {
                    eprintln!("Failed to create service account from file: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                Ok(Arc::new(service_account) as Arc<dyn TokenProvider>)
            }
            None => {
                let provider = gcp_auth::provider().await.map_err(|e| {
                    eprintln!("Failed to create default token provider: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                Ok(provider)
            }
        }
    }
}
