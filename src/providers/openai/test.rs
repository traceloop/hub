use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::debug;

use super::provider::OpenAIProvider;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::tool_choice::{SimpleToolChoice, ToolChoice};
use crate::models::tool_definition::{FunctionDefinition, ToolDefinition};
use crate::providers::provider::Provider;

async fn save_to_cassette(test_name: &str, response: &Value) {
    let cassettes_dir = PathBuf::from("tests/cassettes/openai");
    std::fs::create_dir_all(&cassettes_dir).ok();

    let cassette_path = cassettes_dir.join(format!("{}.json", test_name));

    let mut interactions = Vec::new();

    if let Ok(content) = fs::read_to_string(&cassette_path) {
        if let Ok(mut existing) = serde_json::from_str::<Vec<Value>>(&content) {
            interactions.append(&mut existing);
        }
    }

    interactions.push(response.clone());

    if let Ok(content) = serde_json::to_string_pretty(&interactions) {
        fs::write(&cassette_path, content).expect("Failed to write cassette");
        debug!(
            "Successfully saved interaction to cassette: {:?}",
            cassette_path
        );
    }
}

fn create_test_provider() -> OpenAIProvider {
    let api_key = if std::env::var("RECORD_MODE").is_ok() {
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY required for recording")
    } else {
        "test_key".to_string()
    };

    OpenAIProvider::new(&ProviderConfig {
        key: "openai".to_string(),
        r#type: crate::types::ProviderType::OpenAI,
        api_key,
        params: HashMap::new(),
    })
}

fn create_model_config() -> ModelConfig {
    ModelConfig {
        key: "gpt-5.1".to_string(),
        r#type: "gpt-5.1".to_string(),
        provider: "openai".to_string(),
        params: HashMap::new(),
    }
}

fn create_weather_tool_definition() -> ToolDefinition {
    ToolDefinition {
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
                            "description": "The city and state, e.g. San Francisco, CA"
                        },
                        "unit": {
                            "type": "string",
                            "enum": ["celsius", "fahrenheit"],
                            "description": "The temperature unit"
                        }
                    },
                    "required": ["location"]
                }))
                .unwrap(),
            ),
            strict: None,
        },
    }
}

#[tokio::test]
async fn test_chat_completions_with_tool_calls() {
    let test_name = "chat_completions_with_tool_calls";
    let is_record_mode = std::env::var("RECORD_MODE").is_ok();

    if !is_record_mode {
        debug!("Running test in cassette validation mode");

        let cassette_path =
            PathBuf::from("tests/cassettes/openai").join(format!("{}.json", test_name));

        assert!(
            cassette_path.exists(),
            "Cassette file does not exist. Run with RECORD_MODE=1 OPENAI_API_KEY=sk-... to create it"
        );

        let cassette_content =
            fs::read_to_string(&cassette_path).expect("Failed to read cassette file");

        let interactions: Vec<Value> =
            serde_json::from_str(&cassette_content).expect("Failed to parse cassette JSON");

        assert_eq!(
            interactions.len(),
            2,
            "Cassette should have exactly 2 interactions (tool request + final answer)"
        );

        let first_response = &interactions[0];
        assert!(first_response["id"].is_string(), "Response ID missing");
        assert!(
            first_response["choices"].is_array(),
            "Choices array missing"
        );

        let first_message = &first_response["choices"][0]["message"];
        assert_eq!(
            first_message["role"].as_str().unwrap(),
            "assistant",
            "First response should be from assistant"
        );
        assert!(
            first_message["tool_calls"].is_array(),
            "First response should have tool_calls"
        );

        let tool_calls = first_message["tool_calls"].as_array().unwrap();
        assert!(!tool_calls.is_empty(), "Should have at least one tool call");

        let tool_call = &tool_calls[0];
        assert!(
            tool_call["id"].is_string() && !tool_call["id"].as_str().unwrap().is_empty(),
            "Tool call must have a non-empty id"
        );
        assert_eq!(
            tool_call["type"].as_str().unwrap(),
            "function",
            "Tool call type should be 'function'"
        );
        assert_eq!(
            tool_call["function"]["name"].as_str().unwrap(),
            "get_weather",
            "Tool call should be for get_weather function"
        );

        let second_response = &interactions[1];
        assert!(second_response["id"].is_string(), "Response ID missing");
        assert!(
            second_response["choices"].is_array(),
            "Choices array missing"
        );

        let second_message = &second_response["choices"][0]["message"];
        assert_eq!(
            second_message["role"].as_str().unwrap(),
            "assistant",
            "Second response should be from assistant"
        );
        assert!(
            second_message["content"].is_string(),
            "Second response should have content"
        );

        debug!("Cassette validation passed!");
        return;
    }

    debug!("Running test in RECORD mode");

    let provider = create_test_provider();
    let model_config = create_model_config();

    // FIRST API CALL: User asks question, assistant should request tool
    let request_1 = ChatCompletionRequest {
        model: "gpt-5.1".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "What's the weather in San Francisco?".to_string(),
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        }],
        temperature: Some(0.7),
        top_p: None,
        n: None,
        stream: Some(false),
        stop: None,
        max_tokens: Some(100),
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        tools: Some(vec![create_weather_tool_definition()]),
        tool_choice: Some(ToolChoice::Simple(SimpleToolChoice::Auto)),
        parallel_tool_calls: None,
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
        reasoning: None,
    };

    let response_1 = provider
        .chat_completions(request_1, &model_config)
        .await
        .expect("First API call failed");

    let tool_calls = match &response_1 {
        ChatCompletionResponse::NonStream(completion) => {
            save_to_cassette(test_name, &serde_json::to_value(completion).unwrap()).await;

            assert!(
                !completion.choices.is_empty(),
                "No choices in first response"
            );
            let message = &completion.choices[0].message;

            assert!(
                message.tool_calls.is_some(),
                "First response should have tool_calls"
            );

            let tool_calls = message.tool_calls.as_ref().unwrap();
            assert!(!tool_calls.is_empty(), "Should have at least one tool call");
            assert!(
                !tool_calls[0].id.is_empty(),
                "Tool call must have a non-empty id"
            );

            debug!(
                "First API call successful, tool_call_id: {}",
                tool_calls[0].id
            );
            tool_calls.clone()
        }
        ChatCompletionResponse::Stream(_) => {
            panic!("Unexpected stream response");
        }
    };

    // SECOND API CALL: Submit tool result with tool_call_id, get final answer
    let request_2 = ChatCompletionRequest {
        model: "gpt-5.1".to_string(),
        messages: vec![
            // Original user message
            ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(
                    "What's the weather in San Francisco?".to_string(),
                )),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            },
            ChatCompletionMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
                refusal: None,
            },
            ChatCompletionMessage {
                role: "tool".to_string(),
                content: Some(ChatMessageContent::String(
                    r#"{"temperature": 65, "unit": "fahrenheit", "condition": "sunny"}"#
                        .to_string(),
                )),
                name: Some("get_weather".to_string()),
                tool_calls: None,
                tool_call_id: Some(tool_calls[0].id.clone()), // CRITICAL: Must match the id from tool_calls
                refusal: None,
            },
        ],
        temperature: Some(0.7),
        top_p: None,
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
        reasoning: None,
    };

    let response_2 = provider
        .chat_completions(request_2, &model_config)
        .await
        .expect("Second API call failed");

    match &response_2 {
        ChatCompletionResponse::NonStream(completion) => {
            save_to_cassette(test_name, &serde_json::to_value(completion).unwrap()).await;

            assert!(
                !completion.choices.is_empty(),
                "No choices in second response"
            );
            let message = &completion.choices[0].message;

            assert!(
                message.content.is_some(),
                "Second response should have content"
            );

            debug!("Second API call successful!");
            debug!("Test completed successfully! Cassette saved with 2 interactions.");
        }
        ChatCompletionResponse::Stream(_) => {
            panic!("Unexpected stream response");
        }
    }
}
