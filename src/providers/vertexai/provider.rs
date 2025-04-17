use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{
    Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse,
};
use crate::models::streaming::ChatCompletionChunk;
use crate::models::usage::Usage;
use crate::providers::provider::Provider;
use super::models::{
    ContentPart, GeminiCandidate, GeminiChatRequest, GeminiChatResponse, GeminiContent,
    UsageMetadata, VertexAIStreamChunk,
};
use axum::async_trait;
use axum::http::StatusCode;
use reqwest::Client;
use reqwest_streams::JsonStreamResponse;
use reqwest_streams::error::{StreamBodyError, StreamBodyKind};
use serde_json::json;
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};
use futures::StreamExt;
use tracing::{debug, error, info};

const STREAM_BUFFER_SIZE: usize = 8192;

pub struct VertexAIProvider {
    config: ProviderConfig,
    http_client: Client,
    project_id: String,
    location: String,
}

impl VertexAIProvider {
    fn validate_location(location: &str) -> String {
        // Only allow alphanumeric and hyphen characters
        let sanitized: String = location
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect();

        if sanitized.is_empty() {
            "us-central1".to_string()  // Default if invalid
        } else {
            sanitized
        }
    }

    async fn get_auth_token(&self) -> Result<String, StatusCode> {
        debug!("Getting auth token...");
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
}

#[async_trait]
impl Provider for VertexAIProvider {
    fn new(config: &ProviderConfig) -> Self {
        let project_id = config
            .params
            .get("project_id")
            .expect("project_id is required for VertexAI provider")
            .to_string();
        let location = config
            .params
            .get("location")
            .map(|l| Self::validate_location(l))
            .unwrap_or_else(|| "us-central1".to_string());

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
        let auth_token = self.get_auth_token().await?;
        let endpoint_suffix = if payload.stream.unwrap_or(false) {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        
        let endpoint = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:{}",
            self.location, self.project_id, self.location, payload.model, endpoint_suffix
        );

        let response = self
            .http_client
            .post(&endpoint)
            .bearer_auth(auth_token)
            .json(&GeminiChatRequest::from(payload.clone()))
            .send()
            .await
            .map_err(|e| {
                error!("VertexAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        debug!("Response status: {}", status);

        if status.is_success() {
            if payload.stream.unwrap_or(false) {
                let model = payload.model.clone();
                let stream = response
                    .json_array_stream::<VertexAIStreamChunk>(STREAM_BUFFER_SIZE)
                    .map(move |result| {
                        result.map(|chunk| {
                            let mut completion_chunk: ChatCompletionChunk = chunk.into();
                            completion_chunk.model = model.clone();
                            completion_chunk
                        }).map_err(|e| {
                            StreamBodyError::new(StreamBodyKind::CodecError, Some(Box::new(e)), None)
                        })
                    });

                Ok(ChatCompletionResponse::Stream(Box::pin(stream)))
            } else {
                let response_text = response.text().await.map_err(|e| {
                    error!("Failed to get response text: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                debug!("Response body: {}", response_text);

                // Parse the response as a JSON array
                let responses: Vec<serde_json::Value> = serde_json::from_str(&response_text)
                    .map_err(|e| {
                        error!("Failed to parse response as JSON array: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                // Get the last response which contains the complete message and usage metadata
                let final_response = responses.last().ok_or_else(|| {
                    error!("No valid response chunks found");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

                // Combine all text parts from all responses
                let full_text = responses
                    .iter()
                    .filter_map(|resp| {
                        resp.get("candidates")
                            .and_then(|candidates| candidates.get(0))
                            .and_then(|candidate| candidate.get("content"))
                            .and_then(|content| content.get("parts"))
                            .and_then(|parts| parts.get(0))
                            .and_then(|part| part.get("text"))
                            .and_then(|text| text.as_str())
                            .map(String::from)
                    })
                    .collect::<Vec<String>>()
                    .join("");

                // Create a GeminiChatResponse with the combined text
                let gemini_response = GeminiChatResponse {
                    candidates: vec![GeminiCandidate {
                        content: GeminiContent {
                            role: "model".to_string(),
                            parts: vec![ContentPart { text: full_text }],
                        },
                        finish_reason: final_response["candidates"][0]["finishReason"]
                            .as_str()
                            .map(String::from),
                        safety_ratings: None,
                        tool_calls: None,
                    }],
                    usage_metadata: final_response["usageMetadata"].as_object().map(|obj| {
                        UsageMetadata {
                            prompt_token_count: obj
                                .get("promptTokenCount")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0)
                                as i32,
                            candidates_token_count: obj
                                .get("candidatesTokenCount")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0)
                                as i32,
                            total_token_count: obj
                                .get("totalTokenCount")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32,
                        }
                    }),
                };

                Ok(ChatCompletionResponse::NonStream(
                    gemini_response.to_openai(payload.model),
                ))
            }
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("VertexAI API request error: {}", error_text);
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        _payload: CompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        unimplemented!("Text completions are not supported for Vertex AI. Use chat_completions instead.")
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        _model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let auth_token = self.get_auth_token().await?;
        let endpoint = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:predict",
            self.location, self.project_id, self.location, payload.model
        );

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
                    embedding: pred["embeddings"]["values"]
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
            .map_or_else(|| "test-project".to_string(), |v| v.to_string());
        let location = config
            .params
            .get("location")
            .map_or_else(|| "us-central1".to_string(), |v| v.to_string());

        Self {
            config: config.clone(),
            http_client: client,
            project_id,
            location,
        }
    }

   
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent, ChatMessageContentPart};
    use crate::models::tool_choice::{ToolChoice, SimpleToolChoice};
    use crate::models::tool_definition::{ToolDefinition, FunctionDefinition};
    use crate::providers::vertexai::models::{GeminiFunctionCall, GeminiToolCall, GeminiToolChoice};

    #[test]
    fn test_provider_new() {
        // Test with minimum required config
        let mut params = HashMap::new();
        params.insert("project_id".to_string(), "test-project".to_string());
        
        let config = ProviderConfig {
            key: "test-vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key: "".to_string(),
            params,
        };

        let provider = VertexAIProvider::new(&config);
        assert_eq!(provider.project_id, "test-project");
        assert_eq!(provider.location, "us-central1"); // default location
    }

    #[test]
    #[should_panic(expected = "project_id is required")]
    fn test_provider_new_missing_project_id() {
        let config = ProviderConfig {
            key: "test-vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key: "".to_string(),
            params: HashMap::new(),
        };

        VertexAIProvider::new(&config);
    }

    #[test]
    fn test_gemini_request_conversion() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                }
            ],
            temperature: Some(0.7),
            top_p: Some(0.9),
            n: None,
            stream: Some(false),
            stop: None,
            max_tokens: Some(100),
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        let gemini_request = GeminiChatRequest::from(chat_request);
        
        assert_eq!(gemini_request.contents[0].parts[0].text, "Hello");
        assert_eq!(gemini_request.contents[0].role, "user");
        assert_eq!(gemini_request.generation_config.as_ref().unwrap().temperature, Some(0.7));
        assert_eq!(gemini_request.generation_config.as_ref().unwrap().top_p, Some(0.9));
    }

    #[test]
    fn test_gemini_response_conversion() {
        let gemini_response = GeminiChatResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![ContentPart {
                        text: "Hello there!".to_string(),
                    }],
                },
                finish_reason: Some("STOP".to_string()),
                safety_ratings: None,
                tool_calls: None,
            }],
            usage_metadata: Some(UsageMetadata {
                prompt_token_count: 10,
                candidates_token_count: 20,
                total_token_count: 30,
            }),
        };

        let model = "gemini-1.5-flash".to_string();
        let openai_response = gemini_response.to_openai(model.clone());

        assert_eq!(openai_response.model, model);
        match &openai_response.choices[0].message.content {
            Some(ChatMessageContent::String(text)) => assert_eq!(text, "Hello there!"),
            _ => panic!("Expected String content"),
        }
        assert_eq!(openai_response.choices[0].finish_reason, Some("STOP".to_string()));
        assert_eq!(openai_response.usage.prompt_tokens, 10);
        assert_eq!(openai_response.usage.completion_tokens, 20);
        assert_eq!(openai_response.usage.total_tokens, 30);
    }

    #[test]
    fn test_gemini_request_with_tools() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("Hello".to_string())),
                name: None,
                tool_calls: None,
            }],
            temperature: Some(0.7),
            tools: Some(vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "test_function".to_string(),
                    description: Some("Test function".to_string()),
                    parameters: Some(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "test": {
                                "type": "string"
                            }
                        }
                    })).unwrap()),
                    strict: None,
                },
            }]),
            tool_choice: Some(ToolChoice::Simple(SimpleToolChoice::Auto)),
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            parallel_tool_calls: None,
        };

        let gemini_request = GeminiChatRequest::from(chat_request);
        
        assert!(gemini_request.tools.is_some());
        let tools = gemini_request.tools.unwrap();
        assert_eq!(tools[0].function_declarations[0].name, "test_function");
    }

    #[test]
    fn test_gemini_response_with_tool_calls() {
        let gemini_response = GeminiChatResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![ContentPart {
                        text: "Using weather function".to_string(),
                    }],
                },
                finish_reason: Some("STOP".to_string()),
                safety_ratings: None,
                tool_calls: Some(vec![GeminiToolCall {
                    function: GeminiFunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"location":"San Francisco"}"#.to_string(),
                    },
                }]),
            }],
            usage_metadata: Some(UsageMetadata {
                prompt_token_count: 10,
                candidates_token_count: 20,
                total_token_count: 30,
            }),
        };

        let model = "gemini-1.5-flash".to_string();
        let openai_response = gemini_response.to_openai(model.clone());

        assert!(openai_response.choices[0].message.tool_calls.is_some());
        let tool_calls = openai_response.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[0].function.arguments, r#"{"location":"San Francisco"}"#);
    }

    #[test]
    fn test_gemini_request_with_system_message() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: "system".to_string(),
                    content: Some(ChatMessageContent::String("You are a helpful assistant".to_string())),
                    name: None,
                    tool_calls: None,
                },
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                }
            ],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        let gemini_request = GeminiChatRequest::from(chat_request);
        
        // Verify system message is handled correctly
        assert_eq!(gemini_request.contents.len(), 2);
        assert_eq!(gemini_request.contents[0].role, "system");
    }

    #[test]
    fn test_gemini_request_with_array_content() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::Array(vec![
                    ChatMessageContentPart {
                        r#type: "text".to_string(),
                        text: "Part 1".to_string(),
                    },
                    ChatMessageContentPart {
                        r#type: "text".to_string(),
                        text: "Part 2".to_string(),
                    },
                ])),
                name: None,
                tool_calls: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        let gemini_request = GeminiChatRequest::from(chat_request);
        
        assert_eq!(gemini_request.contents[0].parts[0].text, "Part 1 Part 2");
    }

    #[test]
    fn test_invalid_location_format() {
        let mut params = HashMap::new();
        params.insert("project_id".to_string(), "test-project".to_string());
        params.insert("location".to_string(), "invalid@location".to_string());
        
        let config = ProviderConfig {
            key: "test-vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key: "".to_string(),
            params,
        };

        let provider = VertexAIProvider::new(&config);
        assert_eq!(provider.location, "invalidlocation");  // @ should be removed
    }

    #[test]
    fn test_location_validation() {
        assert_eq!(
            VertexAIProvider::validate_location("us-central1"),
            "us-central1"
        );
        assert_eq!(
            VertexAIProvider::validate_location("invalid@location"),
            "invalidlocation"
        );
        assert_eq!(
            VertexAIProvider::validate_location(""),
            "us-central1"
        );
        assert_eq!(
            VertexAIProvider::validate_location("!@#$%^"),
            "us-central1"
        );
    }

    #[test]
    fn test_auth_config_precedence() {
        let mut params = HashMap::new();
        params.insert("project_id".to_string(), "test-project".to_string());
        params.insert("credentials_path".to_string(), "some/path.json".to_string());
        
        let config = ProviderConfig {
            key: "test-vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key: "test-api-key".to_string(),  // Both API key and credentials provided
            params,
        };

        let provider = VertexAIProvider::new(&config);
        // Should prefer API key over credentials path
        assert!(!provider.config.api_key.is_empty());
        assert_eq!(provider.config.api_key, "test-api-key");
        assert!(provider.config.params.contains_key("credentials_path")); // Credentials path should still be preserved
    }

    #[test]
    fn test_auth_config_credentials_only() {
        let mut params = HashMap::new();
        params.insert("project_id".to_string(), "test-project".to_string());
        params.insert("credentials_path".to_string(), "some/path.json".to_string());
        
        let config = ProviderConfig {
            key: "test-vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key: "".to_string(),  // Empty API key
            params,
        };

        let provider = VertexAIProvider::new(&config);
        assert!(provider.config.api_key.is_empty());
        assert_eq!(provider.config.params.get("credentials_path").unwrap(), "some/path.json");
    }

    #[test]
    fn test_empty_message_handling() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: None,  // Empty content
                name: None,
                tool_calls: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        let gemini_request = GeminiChatRequest::from(chat_request);
        assert_eq!(gemini_request.contents[0].parts[0].text, "");
    }

    #[test]
    fn test_tool_choice_none() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("test".to_string())),
                name: None,
                tool_calls: None,
            }],
            tool_choice: Some(ToolChoice::Simple(SimpleToolChoice::None)),
            tools: Some(vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "test_function".to_string(),
                    description: Some("Test function".to_string()),
                    parameters: Some(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "test": {
                                "type": "string"
                            }
                        }
                    })).unwrap()),
                    strict: None,
                },
            }]),
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            parallel_tool_calls: None,
        };

        let gemini_request = GeminiChatRequest::from(chat_request);
        assert!(matches!(gemini_request.tool_choice, Some(GeminiToolChoice::None)));
    }

    #[test]
    fn test_generation_config_limits() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("test".to_string())),
                name: None,
                tool_calls: None,
            }],
            temperature: Some(2.0),  // Out of range
            top_p: Some(1.5),       // Out of range
            max_tokens: Some(100000), // Very large
            n: None,
            stream: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        let gemini_request = GeminiChatRequest::from(chat_request);
        let config = gemini_request.generation_config.unwrap();
        assert_eq!(config.temperature.unwrap(), 2.0);  // Values are passed through as-is
        assert_eq!(config.top_p.unwrap(), 1.5);       // Values are passed through as-is
        // No need to check for bounds as values are passed through unchanged
    }

    #[test]
    fn test_response_error_mapping() {
        let gemini_response = GeminiChatResponse {
            candidates: vec![],  // Empty candidates
            usage_metadata: None,
        };

        let model = "gemini-1.5-flash".to_string();
        let openai_response = gemini_response.to_openai(model);
        assert!(openai_response.choices.is_empty());
        assert_eq!(openai_response.usage.prompt_tokens, 0);
        assert_eq!(openai_response.usage.completion_tokens, 0);
        assert_eq!(openai_response.usage.total_tokens, 0);
    }
}
