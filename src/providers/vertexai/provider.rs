/*
Example Config:
providers:
  - key: vertexai
    type: vertexai
    api_key: api-key
    params:
      location: us-central1
      project_id: gcp-project-id

*/
use crate::config::constants::stream_buffer_size_bytes;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::models::streaming::ChatCompletionChunk;
use crate::providers::provider::Provider;
use axum::async_trait;
use axum::http::StatusCode;
use reqwest::Client;
use reqwest_streams::*;
use serde_json::Value;
use std::env;

pub struct VertexAIProvider {
    config: ProviderConfig,
    http_client: Client,
}

#[async_trait]
impl Provider for VertexAIProvider {
    fn new(config: &ProviderConfig) -> Self {
        let http_client = Client::new();
        Self {
            config: config.clone(),
            http_client,
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
        let location = self.config.params.get("location").ok_or_else(|| {
            eprintln!("Missing 'location' in provider params");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let project_id = self.config.params.get("project_id").ok_or_else(|| {
            eprintln!("Missing 'project_id' in provider params");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}/generateContent",
            location, project_id, location, payload.model
        );

        // Check if API key is available and use it for authentication
        let request_builder = if let Some(api_key) = self.config.params.get("api_key") {
            self.http_client.post(&url).query(&[("key", api_key)])
        } else {
            let auth_token = get_auth_token()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            self.http_client.post(&url).bearer_auth(auth_token)
        };

        let response = request_builder.json(&payload).send().await.map_err(|e| {
            eprintln!("VertexAI API request error: {}", e);
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
                        eprintln!("VertexAI API response error: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })
            }
        } else {
            eprintln!(
                "VertexAI API error: {}",
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string())
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        payload: CompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        let location = self.config.params.get("location").ok_or_else(|| {
            eprintln!("Missing 'location' in provider params");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let project_id = self.config.params.get("project_id").ok_or_else(|| {
            eprintln!("Missing 'project_id' in provider params");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}/generateContent",
            location, project_id, location, payload.model
        );

        // Check for API key and use the appropriate authentication mechanism
        let request_builder = if let Some(api_key) = self.config.params.get("api_key") {
            self.http_client.post(&url).query(&[("key", api_key)])
        } else {
            let auth_token = get_auth_token()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            self.http_client.post(&url).bearer_auth(auth_token)
        };

        let response = request_builder.json(&payload).send().await.map_err(|e| {
            eprintln!("VertexAI API request error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let status = response.status();
        if status.is_success() {
            response.json().await.map_err(|e| {
                eprintln!("VertexAI API response error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })
        } else {
            eprintln!(
                "VertexAI API error: {}",
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string())
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        _model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let location = self.config.params.get("location").ok_or_else(|| {
            eprintln!("Missing 'location' in provider params");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let project_id = self.config.params.get("project_id").ok_or_else(|| {
            eprintln!("Missing 'project_id' in provider params");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}/generateEmbeddings",
            location, project_id, location, payload.model
        );

        // Check for API key and use the appropriate authentication mechanism
        let request_builder = if let Some(api_key) = self.config.params.get("api_key") {
            self.http_client.post(&url).query(&[("key", api_key)])
        } else {
            let auth_token = get_auth_token()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            self.http_client.post(&url).bearer_auth(auth_token)
        };

        let response = request_builder.json(&payload).send().await.map_err(|e| {
            eprintln!("VertexAI API request error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let status = response.status();
        if status.is_success() {
            response.json().await.map_err(|e| {
                eprintln!("VertexAI API response error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })
        } else {
            eprintln!(
                "VertexAI API error: {}",
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string())
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

/// Fetches the Google Cloud OAuth 2.0 token using ADC.
async fn get_auth_token() -> Result<String, anyhow::Error> {
    let token = env::var("GOOGLE_APPLICATION_CREDENTIALS").ok();
    if token.is_some() {
        let response = Client::new()
            .post("https://oauth2.googleapis.com/token")
            .json(&serde_json::json!({
                "grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer",
                "assertion": token.unwrap()
            }))
            .send()
            .await?;

        let json: Value = response.json().await?;
        Ok(json["access_token"].as_str().unwrap().to_string())
    } else {
        let output = tokio::process::Command::new("gcloud")
            .arg("auth")
            .arg("print-access-token")
            .output()
            .await?;
        Ok(String::from_utf8(output.stdout)?)
    }
}
