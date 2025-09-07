use super::models::{GeminiChatRequest, GeminiChatResponse, VertexAIStreamChunk};
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{
    Embedding, Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse,
};
use crate::models::streaming::ChatCompletionChunk;
use crate::models::usage::EmbeddingUsage;
use crate::providers::provider::Provider;
use crate::types::ProviderType;
use async_trait::async_trait;
use axum::http::StatusCode;
use futures::StreamExt;
use reqwest::Client;
use reqwest_streams::JsonStreamResponse;
use reqwest_streams::error::{StreamBodyError, StreamBodyKind};
use serde_json::json;
use tracing::{debug, error};
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};

const STREAM_BUFFER_SIZE: usize = 8192;

pub struct VertexAIProvider {
    config: ProviderConfig,
    http_client: Client,
    project_id: String,
    location: String,
}

impl VertexAIProvider {
    async fn get_auth_token(&self) -> Result<String, StatusCode> {
        debug!("Getting auth token...");

        // Special case for tests - return dummy token when in test mode
        if self
            .config
            .params
            .get("use_test_auth")
            .is_some_and(|v| v == "true")
        {
            debug!("Using test auth mode, returning dummy token");
            return Ok("test-token-for-vertex-ai".to_string());
        }

        if !self.config.api_key.is_empty() {
            debug!("Using API key authentication");
            Ok(self.config.api_key.clone())
        } else {
            debug!("Using service account authentication");
            let key_path = self.config
                .params
                .get("credentials_path")
                .map(|p| p.to_string())
                .or_else(|| std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok())
                .expect("Either api_key, credentials_path in config, or GOOGLE_APPLICATION_CREDENTIALS environment variable must be set");

            debug!("Reading service account key from: {}", key_path);
            let key_json =
                std::fs::read_to_string(key_path).expect("Failed to read service account key file");

            debug!(
                "Service account key file content length: {}",
                key_json.len()
            );
            let sa_key: ServiceAccountKey =
                serde_json::from_str(&key_json).expect("Failed to parse service account key");

            debug!("Successfully parsed service account key");
            let auth = ServiceAccountAuthenticator::builder(sa_key)
                .build()
                .await
                .expect("Failed to create authenticator");

            debug!("Created authenticator, requesting token...");
            let scopes = &["https://www.googleapis.com/auth/cloud-platform"];
            let token = auth.token(scopes).await.map_err(|e| {
                error!("Failed to get access token: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            debug!("Successfully obtained token");
            Ok(token.token().unwrap_or_default().to_string())
        }
    }

    pub fn validate_location(location: &str) -> Result<String, String> {
        let sanitized = location
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>();

        if sanitized.is_empty() || sanitized != location {
            Err(format!(
                "Invalid location provided: '{location}'. Location must contain only alphanumeric characters and hyphens."
            ))
        } else {
            Ok(sanitized)
        }
    }
}

#[async_trait]
impl Provider for VertexAIProvider {
    fn new(config: &ProviderConfig) -> Self {
        let project_id = config
            .params
            .get("project_id")
            .expect("project_id is required for VertexAI provider")
            .to_string();
        let location_str = config
            .params
            .get("location")
            .expect("location is required for VertexAI provider")
            .to_string();

        let location = Self::validate_location(&location_str)
            .expect("Invalid location provided in configuration");

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

    fn r#type(&self) -> ProviderType {
        ProviderType::VertexAI
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        tracing::debug!(
            "🎯 VertexAI provider received request for model: {}",
            payload.model
        );

        // Validate reasoning config if present
        if let Some(reasoning) = &payload.reasoning {
            if let Err(_e) = reasoning.validate() {
                return Err(StatusCode::BAD_REQUEST);
            }
        }

        let auth_token = self.get_auth_token().await?;
        let endpoint_suffix = if payload.stream.unwrap_or(false) {
            "streamGenerateContent"
        } else {
            "generateContent"
        };

        // Determine if we're in test mode
        let is_test_mode = self
            .config
            .params
            .get("use_test_auth")
            .map_or(false, |v| v == "true");

        let endpoint = if is_test_mode {
            // In test mode, use the mock server endpoint
            let test_endpoint = std::env::var("VERTEXAI_TEST_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8080".to_string());
            debug!("Using test endpoint: {}", test_endpoint);
            test_endpoint
        } else {
            // Normal mode, use the real endpoint
            let service_endpoint = format!("{}-aiplatform.googleapis.com", self.location);
            let full_model_path = format!(
                "projects/{}/locations/{}/publishers/google/models/{}",
                self.project_id, self.location, payload.model
            );
            format!("https://{service_endpoint}/v1/{full_model_path}:{endpoint_suffix}")
        };

        let request_body = GeminiChatRequest::from(payload.clone());
        let has_structured_output = request_body
            .generation_config
            .as_ref()
            .map(|config| config.response_schema.is_some())
            .unwrap_or(false);

        tracing::debug!("🌐 Sending request to endpoint: {}", endpoint);

        let serialized_body = serde_json::to_string_pretty(&request_body)
            .unwrap_or_else(|e| format!("Failed to serialize request: {e}"));
        tracing::debug!("📤 Full Request Body:\n{}", serialized_body);

        // Specifically log the generation_config part
        if let Some(gen_config) = &request_body.generation_config {
            tracing::debug!("⚙️ Generation Config: {:?}", gen_config);
            if let Some(thinking_config) = &gen_config.thinking_config {
                tracing::debug!("🧠 ThinkingConfig in request: {:?}", thinking_config);
            }
        }

        let response_result = self
            .http_client
            .post(&endpoint)
            .bearer_auth(auth_token)
            .json(&request_body)
            .send()
            .await;

        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                error!("VertexAI API request failed before getting response: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        let status = response.status();
        debug!("Response status: {}", status);

        if status.is_success() {
            if payload.stream.unwrap_or(false) {
                let model = payload.model.clone();
                let stream = response
                    .json_array_stream::<VertexAIStreamChunk>(STREAM_BUFFER_SIZE)
                    .map(move |result| {
                        result
                            .map(|chunk| {
                                let mut completion_chunk: ChatCompletionChunk = chunk.into();
                                completion_chunk.model = model.clone();
                                completion_chunk
                            })
                            .map_err(|e| {
                                StreamBodyError::new(
                                    StreamBodyKind::CodecError,
                                    Some(Box::new(e)),
                                    None,
                                )
                            })
                    });

                Ok(ChatCompletionResponse::Stream(Box::pin(stream)))
            } else {
                let response_text = response.text().await.map_err(|e| {
                    error!("Failed to get response text: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                debug!("Raw VertexAI Response Body: {}", response_text);

                // In test mode, we may be getting an array directly from the mock server
                // since we saved multiple interactions in a single array
                if is_test_mode && response_text.trim().starts_with('[') {
                    debug!("Test mode detected array response, extracting first item");
                    let array: Vec<serde_json::Value> = serde_json::from_str(&response_text)
                        .map_err(|e| {
                            error!("Failed to parse test response as array: {}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    if let Some(first_item) = array.first() {
                        // Convert the first item back to JSON string
                        let item_str = serde_json::to_string(first_item).unwrap_or_default();
                        debug!("Using first item from array: {}", item_str);

                        // Parse as GeminiChatResponse
                        let gemini_response: GeminiChatResponse = serde_json::from_str(&item_str)
                            .map_err(|e| {
                            error!("Failed to parse test item as GeminiChatResponse: {}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                        return Ok(ChatCompletionResponse::NonStream(
                            gemini_response.to_openai_with_structured_output(
                                payload.model,
                                has_structured_output,
                            ),
                        ));
                    }
                }

                // Regular parsing for normal API responses
                let gemini_response: GeminiChatResponse = serde_json::from_str(&response_text)
                    .map_err(|e| {
                        error!(
                            "Failed to parse response as GeminiChatResponse. Error: {}, Raw Response: {}",
                            e,
                            response_text
                        );
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                Ok(ChatCompletionResponse::NonStream(
                    gemini_response
                        .to_openai_with_structured_output(payload.model, has_structured_output),
                ))
            }
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "VertexAI API request failed with status {}. Error body: {}",
                status, error_text
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        _payload: CompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        unimplemented!(
            "Text completions are not supported for Vertex AI. Use chat_completions instead."
        )
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        _model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let auth_token = self.get_auth_token().await?;

        // Determine if we're in test mode
        let is_test_mode = self
            .config
            .params
            .get("use_test_auth")
            .map_or(false, |v| v == "true");

        let endpoint = if is_test_mode {
            // In test mode, use the mock server endpoint
            let test_endpoint = std::env::var("VERTEXAI_TEST_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8080".to_string());
            debug!("Using test endpoint for embeddings: {}", test_endpoint);
            test_endpoint
        } else {
            // Normal mode, use the real endpoint
            format!(
                "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:predict",
                self.location, self.project_id, self.location, payload.model
            )
        };

        let response = self
            .http_client
            .post(&endpoint)
            .bearer_auth(auth_token)
            .json(&json!({
                "instances": match payload.input {
                    EmbeddingsInput::Single(text) => vec![json!({"content": text})],
                    EmbeddingsInput::Multiple(texts) => texts.into_iter()
                        .map(|text| json!({"content": text}))
                        .collect::<Vec<_>>(),
                    EmbeddingsInput::SingleTokenIds(tokens) => vec![json!({"content": tokens.iter().map(|t| t.to_string()).collect::<Vec<String>>().join(" ")})],
                    EmbeddingsInput::MultipleTokenIds(token_arrays) => token_arrays.into_iter()
                        .map(|tokens| json!({"content": tokens.iter().map(|t| t.to_string()).collect::<Vec<String>>().join(" ")}))
                        .collect::<Vec<_>>(),
                },
                "parameters": {
                    "autoTruncate": true
                }
            }))
            .send()
            .await
            .map_err(|e| {
                error!("VertexAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        debug!("Embeddings response status: {}", status);

        if status.is_success() {
            let response_text = response.text().await.map_err(|e| {
                error!("Failed to get response text: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            debug!("Embeddings response body: {}", response_text);

            // In test mode, we may be getting an array directly from the mock server
            // since we saved multiple interactions in a single array
            if is_test_mode && response_text.trim().starts_with('[') {
                debug!("Test mode detected array response for embeddings, extracting first item");
                let array: Vec<serde_json::Value> =
                    serde_json::from_str(&response_text).map_err(|e| {
                        error!("Failed to parse test response as array: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                if let Some(first_item) = array.first() {
                    // Use the first item from the array as the response
                    return Ok(EmbeddingsResponse {
                        object: "list".to_string(),
                        data: first_item["data"]
                            .as_array()
                            .unwrap_or(&vec![])
                            .iter()
                            .enumerate()
                            .map(|(i, emb)| Embeddings {
                                object: "embedding".to_string(),
                                embedding: Embedding::Float(
                                    emb["embedding"]
                                        .as_array()
                                        .unwrap_or(&vec![])
                                        .iter()
                                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                                        .collect::<Vec<f32>>(),
                                ),
                                index: i,
                            })
                            .collect(),
                        model: payload.model,
                        usage: EmbeddingUsage {
                            prompt_tokens: Some(0),
                            total_tokens: Some(0),
                        },
                    });
                }
            }

            // Normal processing for regular API responses
            let gemini_response: serde_json::Value =
                serde_json::from_str(&response_text).map_err(|e| {
                    error!("Failed to parse response as JSON: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Extract embeddings from updated response format
            let embeddings = gemini_response["predictions"]
                .as_array()
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
                .iter()
                .enumerate()
                .map(|(i, pred)| Embeddings {
                    object: "embedding".to_string(),
                    embedding: Embedding::Float(
                        pred["embeddings"]["values"]
                            .as_array()
                            .unwrap_or(&vec![])
                            .iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect::<Vec<f32>>(),
                    ),
                    index: i,
                })
                .collect();

            Ok(EmbeddingsResponse {
                object: "list".to_string(),
                data: embeddings,
                model: payload.model,
                usage: EmbeddingUsage {
                    prompt_tokens: Some(0),
                    total_tokens: Some(0),
                },
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("VertexAI API request error: {}", error_text);
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

#[cfg(test)]
impl VertexAIProvider {
    pub fn with_test_client(config: &ProviderConfig, client: reqwest::Client) -> Self {
        let project_id = config
            .params
            .get("project_id")
            .cloned()
            .unwrap_or_else(|| "".to_string());

        let location_str = config
            .params
            .get("location")
            .cloned()
            .unwrap_or_else(|| "".to_string());

        let location = Self::validate_location(&location_str)
            .expect("Invalid location provided for test client configuration");

        Self {
            config: config.clone(),
            http_client: client,
            project_id,
            location,
        }
    }
}
