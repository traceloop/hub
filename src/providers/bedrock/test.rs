#[cfg(test)]
impl crate::providers::bedrock::provider::ClientProvider
    for crate::providers::bedrock::BedrockProvider
{
    // COMMENT OUT THIS BLOCK TO RUN AGAINST ACTUAL AWS SERVICES
    // OR CHANGE YOUR ENVIRONMENT FROM TEST TO PROD
    async fn create_client(&self) -> Result<aws_sdk_bedrockruntime::Client, String> {
        let handler = self
            .config
            .params
            .get("test_response_handler")
            .map(|s| s.as_str());
        let mock_responses = match handler {
            Some("anthropic_chat_completion") => vec![dummy_anthropic_chat_completion_response()],
            Some("ai21_chat_completion") => vec![dummy_ai21_chat_completion_response()],
            Some("ai21_completion") => vec![dummy_ai21_completion_response()],
            Some("titan_chat_completion") => vec![dummy_titan_chat_completion_response()],
            Some("titan_embedding") => vec![dummy_titan_embedding_response()],
            _ => vec![],
        };
        let test_client = create_test_bedrock_client(mock_responses).await;
        Ok(test_client)
    }
}

#[cfg(test)]
fn get_test_provider_config(
    region: &str,
    test_response_handler: &str,
) -> crate::config::models::Provider {
    use std::collections::HashMap;

    let mut params = HashMap::new();
    params.insert("region".to_string(), region.to_string());

    let aws_access_key_id = std::env::var("AWS_ACCESS_KEY_ID").unwrap_or("test_id".to_string());
    let aws_secret_access_key =
        std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or("test_key".to_string());

    params.insert("AWS_ACCESS_KEY_ID".to_string(), aws_access_key_id);
    params.insert("AWS_SECRET_ACCESS_KEY".to_string(), aws_secret_access_key);

    params.insert(
        "test_response_handler".to_string(),
        format!("{}", test_response_handler).to_string(),
    );

    crate::config::models::Provider {
        key: "test_key".to_string(),
        r#type: crate::types::ProviderType::Bedrock,
        api_key: "".to_string(),
        params,
    }
}
#[cfg(test)]
fn get_test_model_config(
    model_type: &str,
    provider_type: &str,
) -> crate::config::models::ModelConfig {
    use std::collections::HashMap;

    let mut params = HashMap::new();
    params.insert("model_provider".to_string(), provider_type.to_string());

    crate::config::models::ModelConfig {
        key: "test-model".to_string(),
        r#type: model_type.to_string(),
        provider: "bedrock".to_string(),
        params,
    }
}

#[cfg(test)]
mod antropic_tests {
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::BedrockProvider;
    use crate::providers::bedrock::test::{get_test_model_config, get_test_provider_config};
    use crate::providers::provider::Provider;

    #[test]
    fn test_bedrock_provider_new() {
        let config = get_test_provider_config("us-east-1", "");
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), crate::types::ProviderType::Bedrock);
    }

    #[tokio::test]
    async fn test_bedrock_provider_chat_completions() {
        let config = get_test_provider_config("us-east-2", "anthropic_chat_completion");
        let provider = BedrockProvider::new(&config);

        let model_config =
            get_test_model_config("us.anthropic.claude-3-haiku-20240307-v1:0", "anthropic");

        let payload = ChatCompletionRequest {
            model: "us.anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(
                    "Tell me a short joke".to_string(),
                )),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: None,
        };

        let result = provider.chat_completions(payload, &model_config).await;

        assert!(result.is_ok(), "Chat completion failed: {:?}", result.err());

        if let Ok(ChatCompletionResponse::NonStream(completion)) = result {
            assert!(!completion.choices.is_empty(), "Expected non-empty choices");
            assert!(
                completion.usage.total_tokens > 0,
                "Expected non-zero token usage"
            );

            let first_choice = &completion.choices[0];
            assert!(
                first_choice.message.content.is_some(),
                "Expected message content"
            );
            assert_eq!(
                first_choice.message.role, "assistant",
                "Expected assistant role"
            );
        }
    }
}

#[cfg(test)]
mod titan_tests {
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::models::embeddings::EmbeddingsInput::Single;
    use crate::models::embeddings::{Embedding, EmbeddingsRequest};
    use crate::providers::bedrock::BedrockProvider;
    use crate::providers::bedrock::test::{get_test_model_config, get_test_provider_config};
    use crate::providers::provider::Provider;

    #[test]
    fn test_titan_provider_new() {
        let config = get_test_provider_config("us-east-2", "");
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), crate::types::ProviderType::Bedrock);
    }

    #[tokio::test]
    async fn test_embeddings() {
        let config = get_test_provider_config("us-east-2", "titan_embedding");
        let provider = BedrockProvider::new(&config);
        let model_config = get_test_model_config("amazon.titan-embed-text-v2:0", "titan");

        let payload = EmbeddingsRequest {
            model: "amazon.titan-embed-text-v2:0".to_string(),
            user: None,
            input: Single("this is where you place your input text".to_string()),
            encoding_format: None,
        };

        let result = provider.embeddings(payload, &model_config).await;
        assert!(
            result.is_ok(),
            "Titan Embeddings generation failed: {:?}",
            result.err()
        );
        let response = result.unwrap();
        assert!(
            !response.data.is_empty(),
            "Expected non-empty embeddings data"
        );
        assert!(
            matches!(&response.data[0].embedding, Embedding::Float(vec) if !vec.is_empty()),
            "Expected non-empty Float embedding vector",
        );
        assert!(
            response.usage.prompt_tokens > Some(0),
            "Expected non-zero token usage"
        );
    }

    #[tokio::test]
    async fn test_chat_completions() {
        let config = get_test_provider_config("us-east-2", "titan_chat_completion");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config("amazon.titan-embed-text-v2:0", "titan");

        let payload = ChatCompletionRequest {
            model: "us.amazon.nova-lite-v1:0".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(
                    "What is the capital of France? Answer in one word.".to_string(),
                )),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: None,
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(result.is_ok(), "Chat completion failed: {:?}", result.err());

        if let Ok(ChatCompletionResponse::NonStream(completion)) = result {
            assert!(!completion.choices.is_empty(), "Expected non-empty choices");
            assert!(
                completion.usage.total_tokens > 0,
                "Expected non-zero token usage"
            );

            let first_choice = &completion.choices[0];
            assert!(
                first_choice.message.content.is_some(),
                "Expected message content"
            );
            assert_eq!(
                first_choice.message.role, "assistant",
                "Expected assistant role"
            );
        }
    }
}

#[cfg(test)]
mod ai21_tests {
    use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
    use crate::models::completion::CompletionRequest;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::BedrockProvider;
    use crate::providers::bedrock::test::{get_test_model_config, get_test_provider_config};
    use crate::providers::provider::Provider;

    #[test]
    fn test_ai21_provider_new() {
        let config = get_test_provider_config("us-east-1", "");
        let provider = BedrockProvider::new(&config);

        assert_eq!(provider.key(), "test_key");
        assert_eq!(provider.r#type(), crate::types::ProviderType::Bedrock);
    }

    #[tokio::test]
    async fn test_ai21_provider_completions() {
        let config = get_test_provider_config("us-east-1", "ai21_completion");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config("ai21.j2-mid-v1", "ai21");

        let payload = CompletionRequest {
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
        assert!(
            response.usage.total_tokens > 0,
            "Expected non-zero token usage"
        );

        let first_choice = &response.choices[0];
        assert!(
            !first_choice.text.is_empty(),
            "Expected non-empty completion text"
        );
        assert!(
            first_choice.logprobs.is_some(),
            "Expected logprobs to be present"
        );
    }

    #[tokio::test]
    async fn test_ai21_provider_chat_completions() {
        let config = get_test_provider_config("us-east-1", "ai21_chat_completion");
        let provider = BedrockProvider::new(&config);

        let model_config = get_test_model_config("ai21.jamba-1-5-mini-v1:0", "ai21");

        let payload = ChatCompletionRequest {
            model: "ai21.jamba-1-5-mini-v1:0".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(
                    "Tell me a short joke".to_string(),
                )),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: Some(0.8),
            top_p: Some(0.8),
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: None,
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(result.is_ok(), "Chat completion failed: {:?}", result.err());

        if let Ok(ChatCompletionResponse::NonStream(completion)) = result {
            assert!(!completion.choices.is_empty(), "Expected non-empty choices");
            assert!(
                completion.usage.total_tokens > 0,
                "Expected non-zero token usage"
            );

            let first_choice = &completion.choices[0];
            assert!(
                first_choice.message.content.is_some(),
                "Expected message content"
            );
            assert_eq!(
                first_choice.message.role, "assistant",
                "Expected assistant role"
            );
        }
    }
}

#[cfg(test)]
mod arn_tests {
    use crate::models::chat::ChatCompletionRequest;
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::providers::bedrock::BedrockProvider;
    use crate::providers::bedrock::test::{get_test_model_config, get_test_provider_config};
    use crate::providers::provider::Provider;

    #[tokio::test]
    async fn test_arn_model_identifier_not_transformed() {
        let config = get_test_provider_config("us-east-1", "anthropic_chat_completion");
        let provider = BedrockProvider::new(&config);

        // Test with full ARN - should not be transformed
        let model_config = get_test_model_config(
            "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.example.test-model-v1:0",
            "anthropic",
        );

        let arn_model =
            "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.example.test-model-v1:0";
        let payload = ChatCompletionRequest {
            model: arn_model.to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(
                    "Tell me a short joke".to_string(),
                )),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: None,
        };

        // The test here is that we don't get a transformation error
        // The mock will handle the actual response
        let result = provider.chat_completions(payload, &model_config).await;

        // Should not fail due to model identifier transformation
        assert!(
            result.is_ok(),
            "ARN model identifier should be handled correctly: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_inference_profile_identifier_not_transformed() {
        let config = get_test_provider_config("us-east-1", "anthropic_chat_completion");
        let provider = BedrockProvider::new(&config);

        // Test with inference profile ID - should not be transformed
        let model_config = get_test_model_config("us-east-1-inference-profile-123", "anthropic");

        let inference_profile_model = "us-east-1-inference-profile-123";
        let payload = ChatCompletionRequest {
            model: inference_profile_model.to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(
                    "Tell me a short joke".to_string(),
                )),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: None,
        };

        let result = provider.chat_completions(payload, &model_config).await;

        // Should not fail due to model identifier transformation
        assert!(
            result.is_ok(),
            "Inference profile identifier should be handled correctly: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_anthropic_with_reasoning_effort() {
        use crate::models::content::ChatMessageContent;

        let model_config = get_test_model_config("claude-3-5-sonnet-v2", "anthropic");

        let provider_config = get_test_provider_config("us-west-2", "anthropic_chat_completion");
        let provider = BedrockProvider::new(&provider_config);

        let payload = ChatCompletionRequest {
            model: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("Hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: Some(crate::models::chat::ReasoningConfig {
                effort: Some("high".to_string()),
                max_tokens: None,
                exclude: None,
            }),
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(
            result.is_ok(),
            "Chat completion with reasoning failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_anthropic_with_reasoning_max_tokens() {
        use crate::models::content::ChatMessageContent;

        let model_config = get_test_model_config("claude-3-5-sonnet-v2", "anthropic");

        let provider_config = get_test_provider_config("us-west-2", "anthropic_chat_completion");
        let provider = BedrockProvider::new(&provider_config);

        let payload = ChatCompletionRequest {
            model: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("Hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: Some(crate::models::chat::ReasoningConfig {
                effort: None,
                max_tokens: Some(1000),
                exclude: None,
            }),
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(
            result.is_ok(),
            "Chat completion with reasoning max_tokens failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_ai21_with_reasoning_effort() {
        use crate::models::content::ChatMessageContent;

        let model_config = get_test_model_config("jamba-1-5-mini", "ai21");

        let provider_config = get_test_provider_config("us-west-2", "ai21_chat_completion");
        let provider = BedrockProvider::new(&provider_config);

        let payload = ChatCompletionRequest {
            model: "ai21.jamba-1-5-mini-v1:0".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("Hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: Some(crate::models::chat::ReasoningConfig {
                effort: Some("medium".to_string()),
                max_tokens: None,
                exclude: None,
            }),
        };

        let result = provider.chat_completions(payload, &model_config).await;
        assert!(
            result.is_ok(),
            "AI21 chat completion with reasoning failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_reasoning_config_validation_both_effort_and_max_tokens() {
        let reasoning_config = crate::models::chat::ReasoningConfig {
            effort: Some("high".to_string()),
            max_tokens: Some(1000),
            exclude: None,
        };

        // Should not error but should log a warning
        let result = reasoning_config.validate();
        assert!(
            result.is_ok(),
            "Validation should succeed when both effort and max_tokens are set"
        );
    }

    #[test]
    fn test_reasoning_config_validation_invalid_effort() {
        let reasoning_config = crate::models::chat::ReasoningConfig {
            effort: Some("invalid".to_string()),
            max_tokens: None,
            exclude: None,
        };

        let result = reasoning_config.validate();
        assert!(
            result.is_err(),
            "Validation should fail for invalid effort value"
        );
        assert!(result.unwrap_err().contains("Invalid effort value"));
    }

    #[test]
    fn test_reasoning_config_validation_empty_effort() {
        let reasoning_config = crate::models::chat::ReasoningConfig {
            effort: Some("".to_string()),
            max_tokens: None,
            exclude: None,
        };

        let result = reasoning_config.validate();
        assert!(
            result.is_err(),
            "Validation should fail for empty effort string"
        );
        assert!(
            result
                .unwrap_err()
                .contains("Effort cannot be empty string")
        );
    }

    #[test]
    fn test_reasoning_config_to_thinking_prompt() {
        use crate::models::chat::ChatCompletionRequest;
        use crate::models::content::ChatCompletionMessage;
        use crate::providers::anthropic::AnthropicChatCompletionRequest;

        // Test that reasoning config no longer adds prompts to system message
        let high_effort_request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(crate::models::content::ChatMessageContent::String(
                    "test".to_string(),
                )),
                name: None,
                tool_calls: None,
                refusal: None,
            }],
            reasoning: Some(crate::models::chat::ReasoningConfig {
                effort: Some("high".to_string()),
                max_tokens: None,
                exclude: None,
            }),
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
        };

        let anthropic_request = AnthropicChatCompletionRequest::from(high_effort_request);
        // System should be None since no system message was provided and reasoning logic removed
        assert!(anthropic_request.system.is_none());
    }

    #[tokio::test]
    async fn test_anthropic_reasoning_prompt_transformation() {
        use crate::providers::anthropic::AnthropicChatCompletionRequest;

        // Test that reasoning config no longer transforms into system prompt for Anthropic
        let payload = ChatCompletionRequest {
            model: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String("Hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: Some(crate::models::chat::ReasoningConfig {
                effort: Some("high".to_string()),
                max_tokens: None,
                exclude: None,
            }),
        };

        // Transform the request to Anthropic format
        let anthropic_request = AnthropicChatCompletionRequest::from(payload);

        // Verify reasoning prompt is no longer included in system message
        assert!(
            anthropic_request.system.is_none(),
            "System message should not be present since reasoning logic was removed"
        );
    }

    #[tokio::test]
    async fn test_anthropic_reasoning_with_existing_system() {
        use crate::providers::anthropic::AnthropicChatCompletionRequest;

        // Test reasoning when there's already a system message
        let payload = ChatCompletionRequest {
            model: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: "system".to_string(),
                    content: Some(ChatMessageContent::String(
                        "You are a helpful assistant.".to_string(),
                    )),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                },
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                },
            ],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            max_completion_tokens: None,
            parallel_tool_calls: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            tool_choice: None,
            tools: None,
            user: None,
            logprobs: None,
            top_logprobs: None,
            response_format: None,
            reasoning: Some(crate::models::chat::ReasoningConfig {
                effort: Some("medium".to_string()),
                max_tokens: None,
                exclude: None,
            }),
        };

        let anthropic_request = AnthropicChatCompletionRequest::from(payload);

        // Verify original system message is preserved (reasoning logic removed)
        assert!(
            anthropic_request.system.is_some(),
            "System message should be present"
        );
        let system_message = anthropic_request.system.unwrap();
        assert_eq!(
            system_message, "You are a helpful assistant.",
            "Should only contain original system message: {}",
            system_message
        );
    }
}

/**

Helper functions for creating test clients and mock responses

*/
#[cfg(test)]
async fn create_test_bedrock_client(
    mock_responses: Vec<aws_smithy_runtime::client::http::test_util::ReplayEvent>,
) -> aws_sdk_bedrockruntime::Client {
    use aws_config::BehaviorVersion;
    use aws_credential_types::Credentials;
    use aws_credential_types::provider::SharedCredentialsProvider;
    use aws_smithy_runtime::client::http::test_util::StaticReplayClient;
    use aws_types::region::Region;

    let replay_client = StaticReplayClient::new(mock_responses);

    let credentials = Credentials::new("test-key", "test-secret", None, None, "testing");
    let credentials_provider = SharedCredentialsProvider::new(credentials);

    let config = aws_config::SdkConfig::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1".to_string()))
        .credentials_provider(credentials_provider)
        .http_client(replay_client)
        .build();

    aws_sdk_bedrockruntime::Client::new(&config)
}
#[cfg(test)]
fn read_response_file(filename: &str) -> Result<String, std::io::Error> {
    use std::fs;
    use std::io::Read;
    use std::path::Path;

    let log_dir = Path::new("src/providers/bedrock/logs");
    let file_path = log_dir.join(filename);

    let mut file = fs::File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    Ok(contents)
}

/**

Mock responses for the Bedrock API

*/
#[cfg(test)]
fn dummy_anthropic_chat_completion_response()
-> aws_smithy_runtime::client::http::test_util::ReplayEvent {
    use aws_smithy_types::body::SdkBody;

    aws_smithy_runtime::client::http::test_util::ReplayEvent::new(
        http::Request::builder()
            .method("POST")
            .uri("https://bedrock-runtime.us-east-2.amazonaws.com/model/us.anthropic.claude-3-haiku-20240307-v1:0/invoke")
            .body(SdkBody::empty())
            .unwrap(),
        http::Response::builder()
            .status(200)
            .body(SdkBody::from(read_response_file("anthropic_claude_3_haiku_20240307_v1_0_chat_completion.json").unwrap()))
            .unwrap(),
    )
}
#[cfg(test)]
fn dummy_ai21_chat_completion_response() -> aws_smithy_runtime::client::http::test_util::ReplayEvent
{
    use aws_smithy_types::body::SdkBody;

    aws_smithy_runtime::client::http::test_util::ReplayEvent::new(
        http::Request::builder()
            .method("POST")
            .uri("https://bedrock-runtime.us-east-1.amazonaws.com/model/ai21.jamba-1-5-mini-v1:0/invoke")
            .body(SdkBody::empty())
            .unwrap(),
        http::Response::builder()
            .status(200)
            .body(SdkBody::from(read_response_file("ai21_jamba_1_5_mini_v1_0_chat_completions.json").unwrap()))
            .unwrap(),
    )
}
#[cfg(test)]
fn dummy_ai21_completion_response() -> aws_smithy_runtime::client::http::test_util::ReplayEvent {
    use aws_smithy_types::body::SdkBody;

    aws_smithy_runtime::client::http::test_util::ReplayEvent::new(
        http::Request::builder()
            .method("POST")
            .uri("https://bedrock-runtime.us-east-1.amazonaws.com/model/ai21.j2-mid-v1/invoke")
            .body(SdkBody::empty())
            .unwrap(),
        http::Response::builder()
            .status(200)
            .body(SdkBody::from(
                read_response_file("ai21_j2_mid_v1_completions.json").unwrap(),
            ))
            .unwrap(),
    )
}
#[cfg(test)]
fn dummy_titan_embedding_response() -> aws_smithy_runtime::client::http::test_util::ReplayEvent {
    use aws_smithy_types::body::SdkBody;

    aws_smithy_runtime::client::http::test_util::ReplayEvent::new(
        http::Request::builder()
            .method("POST")
            .uri("https://bedrock-runtime.us-east-2.amazonaws.com/model/amazon.titan-embed-text-v2:0/invoke")
            .body(SdkBody::empty())
            .unwrap(),
        http::Response::builder()
            .status(200)
            .body(SdkBody::from(read_response_file("amazon_titan_embed_text_v2_0_embeddings.json").unwrap()))
            .unwrap(),
    )
}
#[cfg(test)]
fn dummy_titan_chat_completion_response() -> aws_smithy_runtime::client::http::test_util::ReplayEvent
{
    use aws_smithy_types::body::SdkBody;

    aws_smithy_runtime::client::http::test_util::ReplayEvent::new(
        http::Request::builder()
            .method("POST")
            .uri("https://bedrock-runtime.us-east-2.amazonaws.com/model/us.amazon.nova-lite-v1:0/invoke")
            .body(SdkBody::empty())
            .unwrap(),
        http::Response::builder()
            .status(200)
            .body(SdkBody::from(read_response_file("us_amazon_nova_lite_v1_0_chat_completion.json").unwrap()))
            .unwrap(),
    )
}
