use std::collections::HashMap;
use crate::config::models::{ModelConfig, Provider};

fn get_test_provider_config(region: &str) -> Provider {
    let mut params = HashMap::new();
    params.insert("region".to_string(), region.to_string());

    let aws_access_key_id = std::env::var("AWS_ACCESS_KEY_ID")
        .expect("AWS_ACCESS_KEY_ID must be set for tests");
    let aws_secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .expect("AWS_SECRET_ACCESS_KEY must be set for tests");

    params.insert("AWS_ACCESS_KEY_ID".to_string(), aws_access_key_id);
    params.insert("AWS_SECRET_ACCESS_KEY".to_string(), aws_secret_access_key);


    Provider {
        key: "test_key".to_string(),
        r#type: "".to_string(),
        api_key: "".to_string(),
        params,
    }
}

fn get_test_model_config(model_type: &str, provider_type: &str) ->ModelConfig {
    let mut params = HashMap::new();
    params.insert("model_provider".to_string(), provider_type.to_string());

    ModelConfig {
        key: "test-model".to_string(),
        r#type: model_type.to_string(),
        provider: "bedrock".to_string(),
        params,
    }
}

#[cfg(test)]
mod antropic_tests {
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::providers::bedrock::BedrockProvider;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::test::{get_test_model_config, get_test_provider_config};
    use crate::providers::provider::Provider;



    #[test]
    fn test_bedrock_provider_new() {
        let config = get_test_provider_config("us-east-1");
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), "bedrock");
    }

    #[tokio::test]
    async fn test_bedrock_provider_chat_completions() {
        let config = get_test_provider_config("us-east-2");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config(
            "us.anthropic.claude-3-haiku-20240307-v1:0",
            "anthropic"
        );

        let payload = ChatCompletionRequest {
            model: "us.anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Tell me a short joke".to_string())),
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
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
        };

        let result = provider
            .chat_completions(payload, &model_config).await;

        assert!(result.is_ok(), "Chat completion failed: {:?}", result.err());

        if let Ok(ChatCompletionResponse::NonStream(completion)) = result {
            assert!(!completion.choices.is_empty(), "Expected non-empty choices");
            assert!(completion.usage.total_tokens > 0, "Expected non-zero token usage");

            let first_choice = &completion.choices[0];
            assert!(first_choice.message.content.is_some(), "Expected message content");
            assert_eq!(first_choice.message.role, "assistant", "Expected assistant role");
        }
    }
}

#[cfg(test)]
mod titan_tests {
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::providers::bedrock::BedrockProvider;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::test::{get_test_model_config, get_test_provider_config};
    use crate::providers::provider::Provider;
    use crate::models::embeddings::EmbeddingsInput::Single;
    use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};


    #[test]
    fn test_titan_provider_new() {
        let config = get_test_provider_config("us-east-2");
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), "bedrock");
    }

    #[tokio::test]
    async fn test_embeddings(){

        let config = get_test_provider_config("us-east-2");
        let provider = BedrockProvider::new(&config);
        let model_config = get_test_model_config(
            "amazon.titan-embed-text-v2:0",
            "titan"
        );

        let payload = EmbeddingsRequest {
            model: "amazon.titan-embed-text-v2:0".to_string(),
            user: None,
            input: Single("this is where you place your input text".to_string()),
            encoding_format: None,
        };

        let result = provider.embeddings(payload, &model_config).await;
        assert!(result.is_ok(), "Titan Embeddings generation failed: {:?}", result.err());
        let response = result.unwrap();
        assert!(!response.data.is_empty(), "Expected non-empty embeddings data");
        assert!(!response.data[0].embedding.is_empty(), "Expected non-empty embedding vector");
        assert!(response.usage.prompt_tokens > 0, "Expected non-zero token usage");
    }

    #[tokio::test]
    async fn test_embeddings_invalid_input() {
        let config = get_test_provider_config("us-east-2");
        let provider = BedrockProvider::new(&config);
        let model_config = get_test_model_config(
            "amazon.titan-embed-text-v2:0",
            "titan"
        );

        let payload = EmbeddingsRequest {
            model: model_config.r#type.clone(),
            input: EmbeddingsInput::Single("".to_string()),
            user: None,
            encoding_format: None,
        };

        let result = provider.embeddings(payload, &model_config).await;
        assert!(result.is_err(), "Expected error for empty input");
    }

    #[tokio::test]
    async fn test_chat_completions() {
        let config = get_test_provider_config("us-east-2");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config(
            "amazon.titan-embed-text-v2:0",
            "titan"
        );

        let payload = ChatCompletionRequest {
            model: "us.amazon.nova-lite-v1:0".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String(
                        "What is the capital of France? Answer in one word.".to_string()
                    )),
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
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(result.is_ok(), "Chat completion failed: {:?}", result.err());

        if let Ok(ChatCompletionResponse::NonStream(completion)) = result {
            assert!(!completion.choices.is_empty(), "Expected non-empty choices");
            assert!(completion.usage.total_tokens > 0, "Expected non-zero token usage");

            let first_choice = &completion.choices[0];
            assert!(first_choice.message.content.is_some(), "Expected message content");
            assert_eq!(first_choice.message.role, "assistant", "Expected assistant role");
        }
    }

    #[tokio::test]
    async fn test_chat_completions_invalid_model() {
        let config = get_test_provider_config("us-east-2");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config(
            "amazon.titan-embed-text-v2:0",
            "titan"
        );

        let payload = ChatCompletionRequest {
            model: model_config.r#type.clone(),
            messages: vec![
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String(
                        "What is the capital of France? Answer in one word.".to_string()
                    )),
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
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(result.is_err(), "Expected error for invalid model");
    }
}

#[cfg(test)]
mod ai21_tests {
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::providers::bedrock::BedrockProvider;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::test::{get_test_model_config, get_test_provider_config};
    use crate::providers::provider::Provider;
    use crate::models::completion::CompletionRequest;


    #[test]
    fn test_ai21_provider_new() {
        let config = get_test_provider_config("us-east-1");
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), "bedrock");
    }

    #[tokio::test]
    async fn test_ai21_provider_completions() {
        let config = get_test_provider_config("us-east-1");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config(
            "ai21.j2-mid-v1",
            "ai21"
        );

        let payload = CompletionRequest{
            model: "ai21.j2-mid-v1".to_string(),
            prompt: "Tell me a joke".to_string(),
            suffix: None,
            max_tokens: Some(400),
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            logprobs: None,
            echo: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            best_of: None,
            logit_bias: None,
            user: None,
        };

        let result = provider.completions(payload, &model_config).await;
        assert!(result.is_ok(), "Completion failed: {:?}", result.err());

        let response = result.unwrap();
        assert!(!response.choices.is_empty(), "Expected non-empty choices");
        assert!(response.usage.total_tokens > 0, "Expected non-zero token usage");

        let first_choice = &response.choices[0];
        assert!(!first_choice.text.is_empty(), "Expected non-empty completion text");
        assert!(first_choice.logprobs.is_some(), "Expected logprobs to be present");
    }

    #[tokio::test]
    async fn test_ai21_provider_chat_completions() {
        let config = get_test_provider_config("us-east-1");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config(
            "ai21.jamba-1-5-mini-v1:0",
            "ai21"
        );

        let payload = ChatCompletionRequest {
            model: "ai21.jamba-1-5-mini-v1:0".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String(
                        "Tell me a short joke".to_string()
                    )),
                    name: None,
                    tool_calls: None,
                }
            ],
            temperature: Some(0.8),
            top_p: Some(0.8),
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(result.is_ok(), "Chat completion failed: {:?}", result.err());

        if let Ok(ChatCompletionResponse::NonStream(completion)) = result {
            assert!(!completion.choices.is_empty(), "Expected non-empty choices");
            assert!(completion.usage.total_tokens > 0, "Expected non-zero token usage");

            let first_choice = &completion.choices[0];
            assert!(first_choice.message.content.is_some(), "Expected message content");
            assert_eq!(first_choice.message.role, "assistant", "Expected assistant role");
        }
    }
}