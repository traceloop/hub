use serde_json::json;
use std::path::PathBuf;
use surf::{Client, Config};
use surf_vcr::{VcrMiddleware, VcrMode};

use super::provider::VertexAIProvider;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage};
use crate::models::completion::CompletionRequest;
use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
use crate::providers::provider::Provider;

fn setup_test_client() -> Client {
    let vcr = VcrMiddleware::new(PathBuf::from("tests/cassettes/vertexai"), VcrMode::Replay);

    Config::new().add(vcr).try_into().unwrap()
}

fn create_test_provider() -> VertexAIProvider {
    let config = ProviderConfig {
        key: "test-vertexai".to_string(),
        r#type: "vertexai".to_string(),
        api_key: "test-api-key".to_string(),
        params: HashMap::from([
            ("project_id".to_string(), "test-project".to_string()),
            ("location".to_string(), "us-central1".to_string()),
        ]),
        ..Default::default()
    };

    VertexAIProvider::new(&config)
}

#[tokio::test]
async fn test_chat_completions() {
    let provider = create_test_provider();
    let model_config = ModelConfig {
        key: "gemini-1.5-flash".to_string(),
        r#type: "gemini-1.5-flash".to_string(),
        provider: "vertexai".to_string(),
        ..Default::default()
    };

    let request = ChatCompletionRequest {
        model: "gemini-1.5-flash".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: Some("Hello, how are you?".to_string()),
            function_call: None,
            tool_calls: None,
            name: None,
            tool_call_id: None,
        }],
        temperature: Some(0.7),
        top_p: Some(0.9),
        n: Some(1),
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

    let response = provider
        .chat_completions(request, &model_config)
        .await
        .unwrap();
    match response {
        ChatCompletionResponse::NonStream(resp) => {
            assert_eq!(resp["object"], "chat.completion");
            assert!(!resp["choices"].as_array().unwrap().is_empty());
        }
        _ => panic!("Expected non-stream response"),
    }
}

#[tokio::test]
async fn test_completions() {
    let provider = create_test_provider();
    let model_config = ModelConfig {
        key: "gemini-1.5-flash".to_string(),
        r#type: "gemini-1.5-flash".to_string(),
        provider: "vertexai".to_string(),
        ..Default::default()
    };

    let request = CompletionRequest {
        model: "gemini-1.5-flash".to_string(),
        prompt: "Once upon a time".to_string(),
        suffix: None,
        best_of: None,
        logit_bias: None,
        temperature: Some(0.7),
        top_p: Some(0.9),
        n: Some(1),
        stream: Some(false),
        stop: None,
        max_tokens: Some(100),
        presence_penalty: None,
        frequency_penalty: None,
        logprobs: None,
        echo: None,
        user: None,
    };

    let response = provider.completions(request, &model_config).await.unwrap();
    assert_eq!(response.object, "text_completion");
    assert!(!response.choices.is_empty());
}

#[tokio::test]
async fn test_embeddings() {
    let provider = create_test_provider();
    let model_config = ModelConfig {
        key: "textembedding-gecko".to_string(),
        r#type: "textembedding-gecko".to_string(),
        provider: "vertexai".to_string(),
        ..Default::default()
    };

    let request = EmbeddingsRequest {
        model: "textembedding-gecko".to_string(),
        input: EmbeddingsInput::Multiple(vec!["This is a test sentence.".to_string()]),
        user: None,
        encoding_format: Some("float".to_string()),
    };

    let response = provider.embeddings(request, &model_config).await.unwrap();
    assert_eq!(response.object, "list");
    assert!(!response.data.is_empty());
    assert!(response.data[0]["embedding"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_chat_completions_with_tools() {
    let provider = create_test_provider();
    let model_config = ModelConfig {
        key: "gemini-1.5-flash".to_string(),
        r#type: "gemini-1.5-flash".to_string(),
        provider: "vertexai".to_string(),
        ..Default::default()
    };

    let request = ChatCompletionRequest {
        model: "gemini-1.5-flash".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: Some("What's the weather in San Francisco?".to_string()),
            function_call: None,
            tool_calls: None,
            name: None,
            tool_call_id: None,
        }],
        temperature: Some(0.7),
        top_p: Some(0.9),
        n: Some(1),
        stream: Some(false),
        stop: None,
        max_tokens: Some(100),
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        tools: Some(vec![json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get the current weather in a location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The location to get weather for"
                        }
                    },
                    "required": ["location"]
                }
            }
        })]),
        tool_choice: Some(json!("auto")),
        parallel_tool_calls: Some(1),
    };

    let response = provider
        .chat_completions(request, &model_config)
        .await
        .unwrap();
    match response {
        ChatCompletionResponse::NonStream(resp) => {
            assert_eq!(resp["object"], "chat.completion");
            let choices = resp["choices"].as_array().unwrap();
            assert!(!choices.is_empty());

            let first_choice = &choices[0];
            let message = first_choice["message"].as_object().unwrap();

            // Verify tool calls are present
            if let Some(tool_calls) = message.get("tool_calls") {
                let tool_calls = tool_calls.as_array().unwrap();
                assert!(!tool_calls.is_empty());

                let first_call = &tool_calls[0];
                assert_eq!(first_call["type"], "function");

                let function = first_call["function"].as_object().unwrap();
                assert_eq!(function["name"], "get_weather");
                assert!(function["arguments"]
                    .as_str()
                    .unwrap()
                    .contains("San Francisco"));
            }
        }
        _ => panic!("Expected non-stream response"),
    }
}
