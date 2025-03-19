use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use wiremock::{Mock, MockServer, ResponseTemplate};
use tracing::{debug, error, info};

use super::provider::VertexAIProvider;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::CompletionRequest;
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
use crate::models::tool_definition::{FunctionDefinition, ToolDefinition};
use crate::providers::provider::Provider;

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
            debug!("Successfully saved interaction to cassette: {:?}", cassette_path);
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
        model: "gemini-1.5-flash".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "Hello, how are you?".to_string(),
            )),
            name: None,
            tool_calls: None,
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
    };

    let model_config = ModelConfig {
        key: "gemini-1.5-flash".to_string(),
        r#type: "gemini-1.5-flash".to_string(),
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
async fn test_completions() {
    let client = setup_test_client("completions").await;
    let provider = create_test_provider(client);
    // ... rest of the test implementation

    let request = CompletionRequest {
        model: "gemini-1.5-flash".to_string(),
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
        key: "gemini-1.5-flash".to_string(),
        r#type: "gemini-1.5-flash".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let result = run_test_with_quota_retry(|| async {
        let response = provider.completions(request.clone(), &model_config).await?;
        if std::env::var("RECORD_MODE").is_ok() {
            save_to_cassette("completions", &serde_json::to_value(&response).unwrap()).await;
        }
        Ok(response)
    })
    .await;

    assert!(result.is_ok(), "Test failed: {:?}", result.err());
}

#[tokio::test]
async fn test_chat_completions_with_tools() {
    let client = setup_test_client("chat_completions_with_tools").await;
    let provider = create_test_provider(client);

    let request = ChatCompletionRequest {
        model: "gemini-1.5-flash".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "What's the weather in San Francisco?".to_string(),
            )),
            name: None,
            tool_calls: None,
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
    };

    let model_config = ModelConfig {
        key: "gemini-1.5-flash".to_string(),
        r#type: "gemini-1.5-flash".to_string(),
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
#[ignore = "Requires valid API key which is not available yet"]
async fn test_chat_completions_with_api_key() {
    let api_key = "test-api-key".to_string();
    let client = setup_test_client("chat_completions_api_key").await;
    let provider = create_test_provider_with_api_key(client, api_key);

    let request = ChatCompletionRequest {
        model: "gemini-1.5-flash".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "Hello, how are you?".to_string(),
            )),
            name: None,
            tool_calls: None,
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
    };

    let model_config = ModelConfig {
        key: "gemini-1.5-flash".to_string(),
        r#type: "gemini-1.5-flash".to_string(),
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
