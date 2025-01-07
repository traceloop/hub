use std::collections::HashMap;

fn get_test_provider_config() -> crate::config::models::Provider {
    let mut params = HashMap::new();
    params.insert("region".to_string(), "us-east-1".to_string());


    crate::config::models::Provider {
        key: "test_key".to_string(),
        r#type: "".to_string(),
        api_key: "".to_string(),
        params,
    }
}

#[cfg(test)]
mod antropic_tests {
    use crate::config::models::{ModelConfig, Provider as ProviderConfig};
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::providers::bedrock::BedrockProvider;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::test::get_test_provider_config;
    use crate::providers::provider::Provider;
    use std::collections::HashMap;



    #[test]
    fn test_bedrock_provider_new() {
        let config = get_test_provider_config();
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), "bedrock");
    }

    #[tokio::test]
    async fn test_bedrock_provider_chat_completions() {
        let config = get_test_provider_config();
        let provider = BedrockProvider::new(&config);

        let model_config = ModelConfig {
            key: "test-model".to_string(),
            r#type: "us.anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            provider: "bedrock".to_string(),
            params: HashMap::new(),
        };

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

        match result {
            Ok(response) => {
                println!("Chat completion successful!");
                match response {
                    ChatCompletionResponse::Stream(stream) => {
                        println!("Received streaming response - stream data available");
                    },
                    ChatCompletionResponse::NonStream(completion) => {
                        let json = serde_json::to_string_pretty(&completion).unwrap_or_else(|e| {
                            format!("Failed to serialize response to JSON: {}", e)
                        });
                        println!("Response JSON:\n{}", json);
                    }
                }
            },
            Err(e) => {
                println!("Error occurred during chat completion: {:?}", e);
                panic!("Chat completion failed with error: {:?}", e);
            }
        }
    }
}

#[cfg(test)]
mod titan_tests {
    use crate::config::models::{ModelConfig, Provider as ProviderConfig};
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::providers::bedrock::BedrockProvider;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::test::get_test_provider_config;
    use crate::providers::provider::Provider;
    use std::collections::HashMap;

    #[test]
    fn test_titan_provider_new() {
        let config = get_test_provider_config();
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), "bedrock");
    }

    #[tokio::test]
    async fn test_titan_provider_chat_completions() {
        let config = get_test_provider_config();
        let provider = BedrockProvider::new(&config);

        let model_config = ModelConfig {
            key: "test-model".to_string(),
            r#type: "us.amazon.nova-lite-v1:0".to_string(),
            provider: "bedrock".to_string(),
            params: HashMap::new(),
        };

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

        match result {
            Ok(response) => {
                println!("Chat completion successful!");
                match response {
                    ChatCompletionResponse::Stream(stream) => {
                        println!("Received streaming response - stream data available");
                    },
                    ChatCompletionResponse::NonStream(completion) => {
                        let json = serde_json::to_string_pretty(&completion).unwrap_or_else(|e| {
                            format!("Failed to serialize response to JSON: {}", e)
                        });
                        println!("Response JSON:\n{}", json);
                    }
                }
            },
            Err(e) => {
                println!("Error occurred during chat completion: {:?}", e);
                panic!("Chat completion failed with error: {:?}", e);
            }
        }


    }
}

#[cfg(test)]
mod ai21_tests {
    use crate::config::models::{ModelConfig, Provider as ProviderConfig};
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::providers::bedrock::BedrockProvider;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::test::get_test_provider_config;
    use crate::providers::provider::Provider;
    use std::collections::HashMap;

    #[test]
    fn test_ai21_provider_new() {
        let config = get_test_provider_config();
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), "bedrock");
    }

    #[tokio::test]
    async fn test_ai21_provider_chat_completions() {
        let config = get_test_provider_config();
        let provider = BedrockProvider::new(&config);

        let model_config = ModelConfig {
            key: "test-model".to_string(),
            r#type: "ai21.jamba-1-5-mini-v1:0".to_string(),
            provider: "bedrock".to_string(),
            params: HashMap::new(),
        };

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

        match result {
            Ok(response) => {
                println!("Chat completion successful!");
                match response {
                    ChatCompletionResponse::Stream(stream) => {
                        println!("Received streaming response - stream data available");
                    },
                    ChatCompletionResponse::NonStream(completion) => {
                        let json = serde_json::to_string_pretty(&completion).unwrap_or_else(|e| {
                            format!("Failed to serialize response to JSON: {}", e)
                        });
                        println!("Response JSON:\n{}", json);
                    }
                }
            },
            Err(e) => {
                println!("Error occurred during chat completion: {:?}", e);
                panic!("Chat completion failed with error: {:?}", e);
            }
        }
    }
}