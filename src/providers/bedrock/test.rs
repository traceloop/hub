#[cfg(test)]
mod tests {
    use crate::config::models::{ModelConfig, Provider as ProviderConfig};
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use std::collections::HashMap;
    use crate::providers::bedrock::BedrockProvider;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::provider::Provider;

    fn get_test_provider_config() -> ProviderConfig {
        let mut params = HashMap::new();
        params.insert("region".to_string(), "us-east-2".to_string());


        ProviderConfig {
            key: "test_key".to_string(),
            r#type: "".to_string(),
            api_key: "".to_string(),
            params,
        }
    }

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
            r#type: "anthropic.claude-v1".to_string(),
            provider: "bedrock".to_string(),
            params: HashMap::new(),
        };

        let payload = ChatCompletionRequest {
            model: "anthropic.claude-3-5-sonnet-20240620-v1:0".to_string(),
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