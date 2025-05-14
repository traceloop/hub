use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::provider::VertexAIProvider;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::CompletionRequest;
use crate::models::content::ChatMessageContentPart;
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
use crate::models::tool_choice::SimpleToolChoice;
use crate::models::tool_choice::ToolChoice;
use crate::models::tool_definition::{FunctionDefinition, ToolDefinition};
use crate::providers::provider::Provider;
use crate::providers::vertexai::models::ContentPart;
use crate::providers::vertexai::models::GeminiCandidate;
use crate::providers::vertexai::models::GeminiChatRequest;
use crate::providers::vertexai::models::GeminiChatResponse;
use crate::providers::vertexai::models::GeminiContent;
use crate::providers::vertexai::models::GeminiFunctionCall;
use crate::providers::vertexai::models::GeminiToolChoice;
use crate::providers::vertexai::models::UsageMetadata;

// Test constants
const TEST_PROJECT_ID: &str = "heavenya";
const TEST_LOCATION: &str = "us-central1";

async fn setup_test_client(test_name: &str) -> reqwest::Client {
    // Create the cassettes directory if it doesn't exist
    let cassettes_dir = PathBuf::from("tests/cassettes/vertexai");
    debug!("Creating cassettes directory at: {:?}", cassettes_dir);

    if let Err(e) = std::fs::create_dir_all(&cassettes_dir) {
        error!("Warning: Directory creation returned: {}", e);
    }

    // Create specific cassette file path
    let cassette_path = cassettes_dir.join(format!("{}.json", test_name));
    debug!("Cassette path: {:?}", cassette_path);

    let is_record_mode = std::env::var("RECORD_MODE").is_ok();
    debug!("Record mode: {}", is_record_mode);

    if is_record_mode {
        // In record mode, create a real client
        debug!("Using real client for recording");
        reqwest::Client::builder()
            .build()
            .expect("Failed to create HTTP client")
    } else {
        // In replay mode, use mock server with saved responses
        if let Ok(cassette_content) = fs::read_to_string(&cassette_path) {
            debug!("Loading cassette from: {:?}", cassette_path);
            let mock_server = MockServer::start().await;

            if let Ok(interactions) = serde_json::from_str::<Vec<Value>>(&cassette_content) {
                for interaction in interactions {
                    // Set up mock based on saved interaction
                    Mock::given(wiremock::matchers::any())
                        .respond_with(ResponseTemplate::new(200).set_body_json(interaction))
                        .mount(&mock_server)
                        .await;
                }
            }

            // Create client pointing to mock server
            reqwest::Client::builder()
                .build()
                .expect("Failed to create HTTP client")
        } else {
            debug!("No cassette found, falling back to record mode");
            reqwest::Client::builder()
                .build()
                .expect("Failed to create HTTP client")
        }
    }
}

// Helper function to save response to cassette
async fn save_to_cassette(test_name: &str, response: &Value) {
    let cassettes_dir = PathBuf::from("tests/cassettes/vertexai");
    let cassette_path = cassettes_dir.join(format!("{}.json", test_name));

    let mut interactions = Vec::new();

    // Load existing interactions if any
    if let Ok(content) = fs::read_to_string(&cassette_path) {
        if let Ok(mut existing) = serde_json::from_str::<Vec<Value>>(&content) {
            interactions.append(&mut existing);
        }
    }

    // Add new interaction
    interactions.push(response.clone());

    // Save updated cassette
    if let Ok(content) = serde_json::to_string_pretty(&interactions) {
        if let Err(e) = fs::write(&cassette_path, content) {
            error!("Error saving cassette: {}", e);
        } else {
            debug!(
                "Successfully saved interaction to cassette: {:?}",
                cassette_path
            );
        }
    }
}

fn create_test_provider(client: reqwest::Client) -> VertexAIProvider {
    let mut params = HashMap::new();
    params.insert("project_id".to_string(), TEST_PROJECT_ID.to_string());
    params.insert("location".to_string(), TEST_LOCATION.to_string());

    // Default to service account auth
    params.insert("auth_type".to_string(), "service_account".to_string());
    params.insert(
        "credentials_path".to_string(),
        std::env::var("VERTEXAI_CREDENTIALS_PATH")
            .unwrap_or_else(|_| "vertexai-key.json".to_string()),
    );

    VertexAIProvider::with_test_client(
        &ProviderConfig {
            key: "vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key: "".to_string(), // Empty API key to force service account auth
            params,
        },
        client,
    )
}

// Separate function for API key tests
fn create_test_provider_with_api_key(client: reqwest::Client, api_key: String) -> VertexAIProvider {
    let mut params = HashMap::new();
    params.insert("project_id".to_string(), TEST_PROJECT_ID.to_string());
    params.insert("location".to_string(), TEST_LOCATION.to_string());
    params.insert("auth_type".to_string(), "api_key".to_string());

    VertexAIProvider::with_test_client(
        &ProviderConfig {
            key: "vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key,
            params,
        },
        client,
    )
}

// Helper function to handle quota errors
async fn run_test_with_quota_retry<F, Fut, T>(test_fn: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, axum::http::StatusCode>>,
{
    let max_retries = 3;
    let retry_delay = std::time::Duration::from_secs(
        std::env::var("RETRY_DELAY")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60),
    );

    for attempt in 0..max_retries {
        match test_fn().await {
            Ok(result) => return Ok(result),
            Err(status) if status == axum::http::StatusCode::TOO_MANY_REQUESTS => {
                if attempt < max_retries - 1 {
                    info!(
                        "Quota exceeded, waiting {} seconds before retry...",
                        retry_delay.as_secs()
                    );
                    tokio::time::sleep(retry_delay).await;
                    continue;
                }
                return Err("Quota exceeded after all retries".into());
            }
            Err(e) => return Err(format!("Test failed with error: {}", e).into()),
        }
    }

    Err("Max retries exceeded".into())
}

#[tokio::test]
async fn test_chat_completions() {
    let client = setup_test_client("chat_completions").await;
    let provider = create_test_provider(client);

    let request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "Hello, how are you?".to_string(),
            )),
            name: None,
            tool_calls: None,
            refusal: None,
        }],
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let model_config = ModelConfig {
        key: "gemini-2.0-flash-exp".to_string(),
        r#type: "gemini-2.0-flash-exp".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let result = run_test_with_quota_retry(|| async {
        let response = provider
            .chat_completions(request.clone(), &model_config)
            .await?;
        if std::env::var("RECORD_MODE").is_ok() {
            match &response {
                ChatCompletionResponse::NonStream(completion) => {
                    save_to_cassette(
                        "chat_completions",
                        &serde_json::to_value(completion).unwrap(),
                    )
                    .await;
                }
                ChatCompletionResponse::Stream(_) => {
                    // Handle streaming response if needed
                }
            }
        }
        Ok(response)
    })
    .await;

    assert!(result.is_ok(), "Test failed: {:?}", result.err());
}

#[tokio::test]
async fn test_embeddings() {
    let client = setup_test_client("embeddings").await;
    let provider = create_test_provider(client);

    let request = EmbeddingsRequest {
        model: "text-embedding-005".to_string(),
        input: EmbeddingsInput::Single("This is a test sentence.".to_string()),
        user: None,
        encoding_format: None,
    };

    let model_config = ModelConfig {
        key: "text-embedding-005".to_string(),
        r#type: "text-embedding-005".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let result = run_test_with_quota_retry(|| async {
        let response = provider.embeddings(request.clone(), &model_config).await?;
        if std::env::var("RECORD_MODE").is_ok() {
            save_to_cassette("embeddings", &serde_json::to_value(&response).unwrap()).await;
        }
        Ok(response)
    })
    .await;

    assert!(result.is_ok(), "Test failed: {:?}", result.err());
}

#[tokio::test]
#[should_panic(
    expected = "Text completions are not supported for Vertex AI. Use chat_completions instead."
)]
async fn test_completions() {
    let client = setup_test_client("completions").await;
    let provider = create_test_provider(client);

    let request = CompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        prompt: "Once upon a time".to_string(),
        suffix: None,
        max_tokens: Some(100),
        temperature: Some(0.7),
        top_p: Some(0.9),
        n: None,
        stream: Some(false),
        logprobs: None,
        echo: None,
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        best_of: None,
        logit_bias: None,
        user: None,
    };

    let model_config = ModelConfig {
        key: "gemini-2.0-flash-exp".to_string(),
        r#type: "gemini-2.0-flash-exp".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    // This should panic with unimplemented message
    let _ = provider.completions(request, &model_config).await;
}

#[tokio::test]
async fn test_chat_completions_with_tools() {
    let _ = tracing_subscriber::fmt::try_init();
    let client = setup_test_client("chat_completions_with_tools").await;
    let provider = create_test_provider(client);

    let request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "What's the weather in San Francisco?".to_string(),
            )),
            name: None,
            tool_calls: None,
            refusal: None,
        }],
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
        tools: Some(vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_weather".to_string(),
                description: Some("Get the current weather in a location".to_string()),
                parameters: Some(
                    serde_json::from_value(json!({
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The location to get weather for"
                            }
                        },
                        "required": ["location"]
                    }))
                    .unwrap(),
                ),
                strict: None,
            },
        }]),
        tool_choice: None,
        parallel_tool_calls: None,
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let model_config = ModelConfig {
        key: "gemini-2.0-flash-exp".to_string(),
        r#type: "gemini-2.0-flash-exp".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let result = run_test_with_quota_retry(|| async {
        let response = provider
            .chat_completions(request.clone(), &model_config)
            .await?;
        if std::env::var("RECORD_MODE").is_ok() {
            match &response {
                ChatCompletionResponse::NonStream(completion) => {
                    save_to_cassette(
                        "chat_completions_with_tools",
                        &serde_json::to_value(completion).unwrap(),
                    )
                    .await;
                }
                ChatCompletionResponse::Stream(_) => {}
            }
        }
        Ok(response)
    })
    .await;

    assert!(result.is_ok(), "Test failed: {:?}", result.err());
}

#[tokio::test]
#[ignore = "Requires valid API key which is not available yet"]
async fn test_chat_completions_with_api_key() {
    let api_key = "test-api-key".to_string();
    let client = setup_test_client("chat_completions_api_key").await;
    let provider = create_test_provider_with_api_key(client, api_key);

    let request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "Hello, how are you?".to_string(),
            )),
            name: None,
            tool_calls: None,
            refusal: None,
        }],
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let model_config = ModelConfig {
        key: "gemini-2.0-flash-exp".to_string(),
        r#type: "gemini-2.0-flash-exp".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let result = run_test_with_quota_retry(|| async {
        let response = provider
            .chat_completions(request.clone(), &model_config)
            .await?;
        if std::env::var("RECORD_MODE").is_ok() {
            match &response {
                ChatCompletionResponse::NonStream(completion) => {
                    save_to_cassette(
                        "chat_completions_api_key",
                        &serde_json::to_value(completion).unwrap(),
                    )
                    .await;
                }
                ChatCompletionResponse::Stream(_) => {}
            }
        }
        Ok(response)
    })
    .await;

    assert!(result.is_ok(), "Test failed: {:?}", result.err());
}

#[test]
#[should_panic(
    expected = "Invalid location provided in configuration: \"Invalid location provided: 'invalid@location'. Location must contain only alphanumeric characters and hyphens.\""
)]
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

    let _provider = VertexAIProvider::new(&config);
}

#[test]
fn test_location_validation() {
    let valid = VertexAIProvider::validate_location("us-central1");
    let invalid = VertexAIProvider::validate_location("invalid@location");
    let empty = VertexAIProvider::validate_location("");
    let special = VertexAIProvider::validate_location("!@#$%^");

    assert_eq!(valid, Ok("us-central1".to_string()));
    assert!(invalid.is_err());
    assert!(empty.is_err());
    assert!(special.is_err());
}

#[test]
fn test_auth_config_precedence() {
    let mut params = HashMap::new();
    params.insert("project_id".to_string(), "test-project".to_string());
    params.insert("credentials_path".to_string(), "some/path.json".to_string());
    params.insert("location".to_string(), "us-central".to_string());

    let config = ProviderConfig {
        key: "test-vertexai".to_string(),
        r#type: "vertexai".to_string(),
        api_key: "test-api-key".to_string(),
        params,
    };

    let _provider = VertexAIProvider::new(&config);
}

#[test]
fn test_auth_config_credentials_only() {
    let mut params = HashMap::new();
    params.insert("project_id".to_string(), "test-project".to_string());
    params.insert("credentials_path".to_string(), "some/path.json".to_string());
    params.insert("location".to_string(), "us-central".to_string());

    let config = ProviderConfig {
        key: "test-vertexai".to_string(),
        r#type: "vertexai".to_string(),
        api_key: "".to_string(),
        params,
    };

    let _provider = VertexAIProvider::new(&config);
}

#[test]
fn test_empty_message_handling() {
    let chat_request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: None,
            name: None,
            tool_calls: None,
            refusal: None,
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let gemini_request = GeminiChatRequest::from(chat_request);
    assert!(gemini_request.contents[0].parts[0].text.is_none());
}

#[test]
fn test_tool_choice_none() {
    let chat_request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("test".to_string())),
            name: None,
            tool_calls: None,
            refusal: None,
        }],
        tool_choice: Some(ToolChoice::Simple(SimpleToolChoice::None)),
        tools: Some(vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "test_function".to_string(),
                description: Some("Test function".to_string()),
                parameters: Some(
                    serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "test": {
                                "type": "string"
                            }
                        }
                    }))
                    .unwrap(),
                ),
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let gemini_request = GeminiChatRequest::from(chat_request);
    assert!(matches!(
        gemini_request.tool_choice,
        Some(GeminiToolChoice::None)
    ));
}

#[test]
fn test_generation_config_limits() {
    let chat_request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("test".to_string())),
            name: None,
            tool_calls: None,
            refusal: None,
        }],
        temperature: Some(2.0),
        top_p: Some(1.5),
        max_tokens: Some(100000),
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let gemini_request = GeminiChatRequest::from(chat_request);
    let config = gemini_request.generation_config.unwrap();
    assert_eq!(config.temperature.unwrap(), 2.0);
    assert_eq!(config.top_p.unwrap(), 1.5);
}

#[test]
fn test_response_error_mapping() {
    let gemini_response = GeminiChatResponse {
        candidates: vec![],
        usage_metadata: None,
    };

    let model = "gemini-2.0-flash-exp".to_string();
    let openai_response = gemini_response.to_openai(model);
    assert!(openai_response.choices.is_empty());
    assert_eq!(openai_response.usage.prompt_tokens, 0);
    assert_eq!(openai_response.usage.completion_tokens, 0);
    assert_eq!(openai_response.usage.total_tokens, 0);
}

#[test]
fn test_provider_new() {
    let mut params = HashMap::new();
    params.insert("project_id".to_string(), "test-project".to_string());
    params.insert("location".to_string(), "us-central1".to_string());

    let config = ProviderConfig {
        key: "test-vertexai".to_string(),
        r#type: "vertexai".to_string(),
        api_key: "".to_string(),
        params,
    };

    let provider = VertexAIProvider::new(&config);
    assert_eq!(provider.r#type(), "vertexai");
    assert_eq!(provider.key(), "test-vertexai");
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
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("Hello".to_string())),
            name: None,
            tool_calls: None,
            refusal: None,
        }],
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let gemini_request = GeminiChatRequest::from(chat_request);

    assert_eq!(
        gemini_request.contents[0].parts[0].text,
        Some("Hello".to_string())
    );
    assert_eq!(gemini_request.contents[0].role, "user");
    assert_eq!(
        gemini_request
            .generation_config
            .as_ref()
            .unwrap()
            .temperature,
        Some(0.7)
    );
    assert_eq!(
        gemini_request.generation_config.as_ref().unwrap().top_p,
        Some(0.9)
    );
}

#[test]
fn test_gemini_response_conversion() {
    let gemini_response = GeminiChatResponse {
        candidates: vec![GeminiCandidate {
            content: GeminiContent {
                role: "model".to_string(),
                parts: vec![ContentPart {
                    text: Some("Hello there!".to_string()),
                    function_call: None,
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

    let model = "gemini-2.0-flash-exp".to_string();
    let openai_response = gemini_response.to_openai(model.clone());

    assert_eq!(openai_response.model, model);
    match &openai_response.choices[0].message.content {
        Some(ChatMessageContent::String(text)) => assert_eq!(text, "Hello there!"),
        _ => panic!("Expected String content"),
    }
    assert_eq!(
        openai_response.choices[0].finish_reason,
        Some("STOP".to_string())
    );
    assert_eq!(openai_response.usage.prompt_tokens, 10);
    assert_eq!(openai_response.usage.completion_tokens, 20);
    assert_eq!(openai_response.usage.total_tokens, 30);
}

#[test]
fn test_gemini_request_with_tools() {
    let chat_request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("Hello".to_string())),
            name: None,
            tool_calls: None,
            refusal: None,
        }],
        temperature: Some(0.7),
        tools: Some(vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "test_function".to_string(),
                description: Some("Test function".to_string()),
                parameters: Some(
                    serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "test": {
                                "type": "string"
                            }
                        }
                    }))
                    .unwrap(),
                ),
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
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
                    text: None,
                    function_call: Some(GeminiFunctionCall {
                        name: "get_weather".to_string(),
                        args: serde_json::json!({
                            "location": "San Francisco"
                        }),
                    }),
                }],
            },
            finish_reason: Some("TOOL_CODE".to_string()),
            safety_ratings: None,
            tool_calls: None,
        }],
        usage_metadata: Some(UsageMetadata {
            prompt_token_count: 10,
            candidates_token_count: 20,
            total_token_count: 30,
        }),
    };

    let model = "gemini-2.0-flash-exp".to_string();
    let openai_response = gemini_response.to_openai(model.clone());

    assert!(openai_response.choices[0].message.tool_calls.is_some());
    let tool_calls = openai_response.choices[0]
        .message
        .tool_calls
        .as_ref()
        .unwrap();
    assert_eq!(tool_calls[0].function.name, "get_weather");
    assert_eq!(
        tool_calls[0].function.arguments,
        r#"{"location":"San Francisco"}"#
    );
}

#[test]
fn test_gemini_request_with_system_message() {
    let chat_request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
        messages: vec![
            ChatCompletionMessage {
                role: "system".to_string(),
                content: Some(ChatMessageContent::String(
                    "You are a helpful assistant".to_string(),
                )),
                name: None,
                tool_calls: None,
                refusal: None,
            },
            ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("Hello".to_string())),
                name: None,
                tool_calls: None,
                refusal: None,
            },
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let gemini_request = GeminiChatRequest::from(chat_request);

    assert_eq!(gemini_request.contents.len(), 2);
    assert_eq!(gemini_request.contents[0].role, "system");
}

#[test]
fn test_gemini_request_with_array_content() {
    let chat_request = ChatCompletionRequest {
        model: "gemini-2.0-flash-exp".to_string(),
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
            refusal: None,
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
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
    };

    let gemini_request = GeminiChatRequest::from(chat_request);

    assert_eq!(
        gemini_request.contents[0].parts[0].text,
        Some("Part 1 Part 2".to_string())
    );
}
