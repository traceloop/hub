use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionChoice, CompletionRequest, CompletionResponse};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{
    Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse,
};
use crate::models::usage::Usage;
use crate::models::vertexai::{
    ContentPart, GeminiCandidate, GeminiChatRequest, GeminiChatResponse, GeminiContent,
    UsageMetadata,
};
use crate::providers::provider::Provider;
use axum::async_trait;
use axum::http::StatusCode;
use reqwest::Client;
use serde_json::json;
use yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey};

pub struct VertexAIProvider {
    config: ProviderConfig,
    http_client: Client,
    project_id: String,
    location: String,
}

impl VertexAIProvider {
    async fn get_auth_token(&self) -> Result<String, StatusCode> {
        println!("Getting auth token...");
        if !self.config.api_key.is_empty() {
            println!("Using API key authentication");
            Ok(self.config.api_key.clone())
        } else {
            println!("Using service account authentication");
            let key_path = self.config
                .params
                .get("credentials_path")
                .map(|p| p.to_string())
                .or_else(|| std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok())
                .expect("Either api_key, credentials_path in config, or GOOGLE_APPLICATION_CREDENTIALS environment variable must be set");

            println!("Reading service account key from: {}", key_path);
            let key_json =
                std::fs::read_to_string(key_path).expect("Failed to read service account key file");

            println!(
                "Service account key file content length: {}",
                key_json.len()
            );
            let sa_key: ServiceAccountKey =
                serde_json::from_str(&key_json).expect("Failed to parse service account key");

            println!("Successfully parsed service account key");
            let auth = ServiceAccountAuthenticator::builder(sa_key)
                .build()
                .await
                .expect("Failed to create authenticator");

            println!("Created authenticator, requesting token...");
            let scopes = &["https://www.googleapis.com/auth/cloud-platform"];
            let token = auth.token(scopes).await.map_err(|e| {
                eprintln!("Failed to get access token: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            println!("Successfully obtained token");
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
        let auth_token = self.get_auth_token().await?;
        let endpoint = format!(
            "https://us-central1-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent",
            self.project_id, self.location, payload.model
        );

        let response = self
            .http_client
            .post(&endpoint)
            .bearer_auth(auth_token)
            .json(&GeminiChatRequest::from_openai(payload.clone()))
            .send()
            .await
            .map_err(|e| {
                eprintln!("VertexAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        println!("Response status: {}", status);

        if status.is_success() {
            if payload.stream.unwrap_or(false) {
                Err(StatusCode::BAD_REQUEST) // Streaming not supported yet
            } else {
                let response_text = response.text().await.map_err(|e| {
                    eprintln!("Failed to get response text: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                println!("Response body: {}", response_text);

                // Parse the response as a JSON array
                let responses: Vec<serde_json::Value> = serde_json::from_str(&response_text)
                    .map_err(|e| {
                        eprintln!("Failed to parse response as JSON array: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                // Get the last response which contains the complete message and usage metadata
                let final_response = responses.last().ok_or_else(|| {
                    eprintln!("No valid response chunks found");
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
            eprintln!("VertexAI API request error: {}", error_text);
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
        let auth_token = self.get_auth_token().await?;
        let endpoint = format!(
            "https://us-central1-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:predict",
            self.project_id, self.location, payload.model
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
                eprintln!("VertexAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();
        println!("Embeddings response status: {}", status);

        if status.is_success() {
            let response_text = response.text().await.map_err(|e| {
                eprintln!("Failed to get response text: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            println!("Embeddings response body: {}", response_text);

            let gemini_response: serde_json::Value =
                serde_json::from_str(&response_text).map_err(|e| {
                    eprintln!("Failed to parse response as JSON: {}", e);
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
            eprintln!("VertexAI API request error: {}", error_text);
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
