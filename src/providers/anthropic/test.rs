use super::models::{
    AnthropicChatCompletionRequest, AnthropicChatCompletionResponse, ContentBlock,
};
use super::provider::AnthropicProvider;
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::tool_choice::{SimpleToolChoice, ToolChoice};
use crate::models::tool_definition::{FunctionDefinition, ToolDefinition};
use crate::providers::provider::Provider;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[allow(unused_imports)]
use tracing::debug;

async fn save_to_cassette(test_name: &str, response: &Value) {
    let cassettes_dir = PathBuf::from("tests/cassettes/anthropic");
    std::fs::create_dir_all(&cassettes_dir).expect("Failed to create cassettes directory");

    let cassette_path = cassettes_dir.join(format!("{}.json", test_name));

    let interactions = vec![response.clone()];

    let content =
        serde_json::to_string_pretty(&interactions).expect("Failed to serialize cassette");
    fs::write(&cassette_path, content).expect("Failed to write cassette");
    debug!(
        "Successfully saved interaction to cassette: {:?}",
        cassette_path
    );
}

fn create_test_provider() -> AnthropicProvider {
    let api_key = if std::env::var("RECORD_MODE").is_ok() {
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY required for recording")
    } else {
        "test_key".to_string()
    };

    AnthropicProvider::new(&ProviderConfig {
        key: "anthropic".to_string(),
        r#type: crate::types::ProviderType::Anthropic,
        api_key,
        params: HashMap::new(),
    })
}

fn create_model_config() -> ModelConfig {
    ModelConfig {
        key: "claude-sonnet-4-20250514".to_string(),
        r#type: "claude-sonnet-4-20250514".to_string(),
        provider: "anthropic".to_string(),
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
async fn test_chat_completions_basic() {
    let test_name = "chat_completions_basic";
    let is_record_mode = std::env::var("RECORD_MODE").is_ok();

    if !is_record_mode {
        debug!("Running test in cassette validation mode");

        let cassette_path =
            PathBuf::from("tests/cassettes/anthropic").join(format!("{}.json", test_name));

        assert!(
            cassette_path.exists(),
            "Cassette file does not exist. Run with RECORD_MODE=1 ANTHROPIC_API_KEY=sk-ant-... to create it"
        );

        let cassette_content =
            fs::read_to_string(&cassette_path).expect("Failed to read cassette file");

        let interactions: Vec<Value> =
            serde_json::from_str(&cassette_content).expect("Failed to parse cassette JSON");

        assert_eq!(
            interactions.len(),
            1,
            "Cassette should have exactly 1 interaction"
        );

        let response = &interactions[0];
        assert!(response["id"].is_string(), "Response ID missing");
        assert!(response["choices"].is_array(), "Choices array missing");
        assert!(
            !response["choices"].as_array().unwrap().is_empty(),
            "Choices array should not be empty"
        );
        assert!(response["usage"].is_object(), "Usage object missing");

        let message = &response["choices"][0]["message"];
        assert_eq!(
            message["role"].as_str().unwrap(),
            "assistant",
            "Response should be from assistant"
        );
        assert!(
            message["content"].is_string(),
            "Content should be a plain string, not an array"
        );

        debug!("Cassette validation passed!");
        return;
    }

    debug!("Running test in RECORD mode");

    let provider = create_test_provider();
    let model_config = create_model_config();

    let request = ChatCompletionRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "Say hello in one sentence.".to_string(),
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        }],
        temperature: Some(0.0),
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

    let response = provider
        .chat_completions(request, &model_config)
        .await
        .expect("API call failed");

    match &response {
        ChatCompletionResponse::NonStream(completion) => {
            save_to_cassette(test_name, &serde_json::to_value(completion).unwrap()).await;

            assert!(!completion.choices.is_empty(), "No choices in response");
            let message = &completion.choices[0].message;
            assert!(message.content.is_some(), "Response should have content");

            debug!("Test completed successfully! Cassette saved.");
        }
        ChatCompletionResponse::Stream(_) => {
            panic!("Unexpected stream response");
        }
    }
}

#[tokio::test]
async fn test_chat_completions_with_tool_calls() {
    let test_name = "chat_completions_with_tool_calls";
    let is_record_mode = std::env::var("RECORD_MODE").is_ok();

    if !is_record_mode {
        debug!("Running test in cassette validation mode");

        let cassette_path =
            PathBuf::from("tests/cassettes/anthropic").join(format!("{}.json", test_name));

        assert!(
            cassette_path.exists(),
            "Cassette file does not exist. Run with RECORD_MODE=1 ANTHROPIC_API_KEY=sk-ant-... to create it"
        );

        let cassette_content =
            fs::read_to_string(&cassette_path).expect("Failed to read cassette file");

        let interactions: Vec<Value> =
            serde_json::from_str(&cassette_content).expect("Failed to parse cassette JSON");

        assert_eq!(
            interactions.len(),
            1,
            "Cassette should have exactly 1 interaction (tool call response)"
        );

        let response = &interactions[0];
        assert!(response["id"].is_string(), "Response ID missing");
        assert!(response["choices"].is_array(), "Choices array missing");
        assert!(
            !response["choices"].as_array().unwrap().is_empty(),
            "Choices array should not be empty"
        );

        let message = &response["choices"][0]["message"];
        assert_eq!(
            message["role"].as_str().unwrap(),
            "assistant",
            "Response should be from assistant"
        );
        assert!(
            message["tool_calls"].is_array(),
            "Response should have tool_calls"
        );

        let tool_calls = message["tool_calls"].as_array().unwrap();
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

        debug!("Cassette validation passed!");
        return;
    }

    debug!("Running test in RECORD mode");

    let provider = create_test_provider();
    let model_config = create_model_config();

    let request = ChatCompletionRequest {
        model: "claude-sonnet-4-20250514".to_string(),
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
        temperature: Some(0.0),
        top_p: None,
        n: None,
        stream: Some(false),
        stop: None,
        max_tokens: Some(200),
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        user: None,
        tools: Some(vec![create_weather_tool_definition()]),
        tool_choice: Some(ToolChoice::Simple(SimpleToolChoice::Required)),
        parallel_tool_calls: None,
        max_completion_tokens: None,
        logprobs: None,
        top_logprobs: None,
        response_format: None,
        reasoning: None,
    };

    let response = provider
        .chat_completions(request, &model_config)
        .await
        .expect("API call failed");

    match &response {
        ChatCompletionResponse::NonStream(completion) => {
            save_to_cassette(test_name, &serde_json::to_value(completion).unwrap()).await;

            assert!(!completion.choices.is_empty(), "No choices in response");
            let message = &completion.choices[0].message;

            assert!(
                message.tool_calls.is_some(),
                "Response should have tool_calls"
            );

            let tool_calls = message.tool_calls.as_ref().unwrap();
            assert!(!tool_calls.is_empty(), "Should have at least one tool call");
            assert!(
                !tool_calls[0].id.is_empty(),
                "Tool call must have a non-empty id"
            );

            debug!("Tool call recorded, id: {}", tool_calls[0].id);
            debug!("Test completed successfully! Cassette saved.");
        }
        ChatCompletionResponse::Stream(_) => {
            panic!("Unexpected stream response");
        }
    }
}

#[test]
fn test_content_block_to_message_text_only() {
    let blocks = vec![
        ContentBlock::Text {
            text: "Hello ".to_string(),
        },
        ContentBlock::Text {
            text: "world!".to_string(),
        },
    ];

    let message: ChatCompletionMessage = blocks.into();
    assert_eq!(message.role, "assistant");

    // Content should be present
    assert!(message.content.is_some());

    // Verify it serializes correctly for OpenAI compatibility
    let json = serde_json::to_value(&message).unwrap();
    let content = &json["content"];

    assert!(content.is_string(), "Content should be a plain string");
    assert_eq!(content.as_str().unwrap(), "Hello world!");

    assert!(message.tool_calls.is_none());
}

#[test]
fn test_content_block_to_message_with_tool_calls() {
    let blocks = vec![
        ContentBlock::Text {
            text: "I'll check the weather.".to_string(),
        },
        ContentBlock::ToolUse {
            id: "toolu_123".to_string(),
            name: "get_weather".to_string(),
            input: json!({"location": "San Francisco"}),
        },
    ];

    let message: ChatCompletionMessage = blocks.into();
    assert_eq!(message.role, "assistant");
    assert!(message.content.is_some());
    assert!(message.tool_calls.is_some());

    let tool_calls = message.tool_calls.unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "toolu_123");
    assert_eq!(tool_calls[0].r#type, "function");
    assert_eq!(tool_calls[0].function.name, "get_weather");
}

#[test]
fn test_content_block_to_message_tool_calls_only() {
    let blocks = vec![ContentBlock::ToolUse {
        id: "toolu_456".to_string(),
        name: "get_weather".to_string(),
        input: json!({"location": "NYC"}),
    }];

    let message: ChatCompletionMessage = blocks.into();
    assert!(message.tool_calls.is_some());

    assert!(
        message.content.is_none(),
        "Tool-only response should have no content"
    );
    assert_eq!(message.tool_calls.as_ref().unwrap().len(), 1);
}

#[test]
fn test_anthropic_response_to_chat_completion() {
    let response = AnthropicChatCompletionResponse {
        id: "msg_123".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        content: vec![ContentBlock::Text {
            text: "Hello!".to_string(),
        }],
        usage: super::models::Usage {
            input_tokens: 10,
            output_tokens: 5,
        },
    };

    let completion: crate::models::chat::ChatCompletion = response.into();
    assert_eq!(completion.id, "msg_123");
    assert_eq!(completion.model, "claude-sonnet-4-20250514");
    assert_eq!(completion.choices.len(), 1);
    assert_eq!(completion.choices[0].index, 0);
    assert_eq!(completion.choices[0].finish_reason.as_deref(), Some("stop"));
    assert_eq!(completion.usage.prompt_tokens, 10);
    assert_eq!(completion.usage.completion_tokens, 5);
    assert_eq!(completion.usage.total_tokens, 15);
}

#[test]
fn test_request_drops_top_p_when_both_temperature_and_top_p_set() {
    let request = ChatCompletionRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("hi".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        }],
        temperature: Some(0.7),
        top_p: Some(0.9),
        n: None,
        stream: None,
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

    let anthropic_request = AnthropicChatCompletionRequest::from(request);
    assert_eq!(anthropic_request.temperature, Some(0.7));
    assert!(
        anthropic_request.top_p.is_none(),
        "top_p should be dropped when temperature is also set"
    );
}

#[test]
fn test_request_preserves_top_p_when_temperature_absent() {
    let request = ChatCompletionRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("hi".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        }],
        temperature: None,
        top_p: Some(0.9),
        n: None,
        stream: None,
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

    let anthropic_request = AnthropicChatCompletionRequest::from(request);
    assert!(anthropic_request.temperature.is_none());
    assert_eq!(
        anthropic_request.top_p,
        Some(0.9),
        "top_p should be preserved when temperature is absent"
    );
}
