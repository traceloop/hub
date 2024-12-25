use dotenv::dotenv;
use futures::StreamExt;
use hub::config::models::{ModelConfig, Provider as ProviderConfig};
use hub::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use hub::models::content::{ChatCompletionMessage, ChatMessageContent};
use hub::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
use hub::providers::provider::Provider;
use hub::providers::vertexai::VertexAIProvider;
use std::collections::HashMap;
use std::env;

async fn create_live_provider() -> VertexAIProvider {
    dotenv().ok();
    let mut params = HashMap::new();

    params.insert(
        "project_id".to_string(),
        env::var("VERTEX_PROJECT_ID").expect("VERTEX_PROJECT_ID environment variable must be set"),
    );
    params.insert(
        "location".to_string(),
        env::var("VERTEX_LOCATION").unwrap_or_else(|_| "us-central1".to_string()),
    );

    if let Ok(creds_path) = env::var("VERTEX_CREDENTIALS_PATH") {
        params.insert("credentials_path".to_string(), creds_path);
    }

    let config = ProviderConfig {
        key: "vertexai".to_string(),
        r#type: "vertexai".to_string(),
        // VertexAI doesn't use API keys
        api_key: "".to_string(),
        params,
    };

    VertexAIProvider::new(&config)
}

fn create_test_model_config() -> ModelConfig {
    ModelConfig {
        key: "test-gemini".to_string(),
        r#type: "gemini-pro".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    }
}

#[tokio::test]
async fn test_chat_completion() {
    let provider = create_live_provider().await;
    let model_config = create_test_model_config();

    let request = ChatCompletionRequest {
        model: "gemini-pro".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "What is the capital of France?".to_string(),
            )),
            name: None,
            tool_calls: None,
        }],
        temperature: Some(0.7),
        stream: None,
        max_tokens: Some(100),
        top_p: None,
        n: None,
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        tool_choice: None,
        tools: None,
        user: None,
        parallel_tool_calls: None,
    };

    let response = provider.chat_completions(request, &model_config).await;
    assert!(response.is_ok(), "Chat completion request failed");

    if let Ok(ChatCompletionResponse::NonStream(completion)) = response {
        assert!(!completion.choices.is_empty(), "No choices in response");
        assert!(
            completion.choices[0].message.content.is_some(),
            "No content in response"
        );
        if let Some(ChatMessageContent::String(content)) = &completion.choices[0].message.content {
            assert!(content.contains("Paris"), "Response should mention Paris");
        }
    }
}

#[tokio::test]
async fn test_streaming_chat_completion() {
    let provider = create_live_provider().await;
    let model_config = create_test_model_config();

    let request = ChatCompletionRequest {
        model: "gemini-pro".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "Write a short poem about coding".to_string(),
            )),
            name: None,
            tool_calls: None,
        }],
        temperature: Some(0.7),
        stream: Some(true),
        max_tokens: Some(100),
        top_p: None,
        n: None,
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        logit_bias: None,
        tool_choice: None,
        tools: None,
        user: None,
        parallel_tool_calls: None,
    };

    let response = provider.chat_completions(request, &model_config).await;
    assert!(response.is_ok(), "Streaming request failed");

    if let Ok(ChatCompletionResponse::Stream(mut stream)) = response {
        let mut chunks_received = 0;
        while let Some(chunk_result) = stream.next().await {
            assert!(chunk_result.is_ok(), "Error receiving stream chunk");
            chunks_received += 1;
        }
        assert!(chunks_received > 0, "No chunks received from stream");
    }
}

#[tokio::test]
async fn test_embeddings() {
    let provider = create_live_provider().await;
    let model_config = ModelConfig {
        key: "test-embeddings".to_string(),
        r#type: "textembedding-gecko".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let request = EmbeddingsRequest {
        model: "textembedding-gecko".to_string(),
        input: EmbeddingsInput::Single("This is a test sentence for embeddings".to_string()),
        user: None,
        encoding_format: None,
    };

    let response = provider.embeddings(request, &model_config).await;
    assert!(response.is_ok(), "Embeddings request failed");

    if let Ok(embeddings) = response {
        assert!(!embeddings.data.is_empty(), "No embeddings in response");
        assert!(
            !embeddings.data[0].embedding.is_empty(),
            "Empty embedding vector"
        );
        assert_eq!(
            embeddings.data[0].embedding.len(),
            768,
            "Incorrect embedding dimensions"
        );
    }
}

#[tokio::test]
async fn test_error_handling() {
    let provider = create_live_provider().await;
    let model_config = create_test_model_config();

    let request = ChatCompletionRequest {
        model: "invalid-model".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("Test".to_string())),
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
        tool_choice: None,
        tools: None,
        user: None,
        parallel_tool_calls: None,
    };

    let response = provider.chat_completions(request, &model_config).await;
    assert!(response.is_err(), "Should fail with invalid model");
}
