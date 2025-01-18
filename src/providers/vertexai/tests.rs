mod tests {
    use crate::config::models::Provider as ProviderConfig;
    use crate::models::chat::{ChatCompletion, ChatCompletionRequest};
    use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
    use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
    use crate::models::tool_definition::{FunctionDefinition, ToolDefinition};
    use crate::providers::provider::Provider;
    use crate::providers::vertexai::models::{
        Content, GenerateContentResponse, Part, UsageMetadata, VertexAIChatCompletionRequest,
        VertexAIChatCompletionResponse, VertexAIEmbeddingsRequest, VertexFunctionCall,
    };
    use crate::providers::vertexai::provider::VertexAIProvider;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_config() -> ProviderConfig {
        let mut params = HashMap::new();
        params.insert("project_id".to_string(), "test-project".to_string());
        params.insert("location".to_string(), "us-central1".to_string());
        params.insert(
            "credentials_path".to_string(),
            "test-credentials.json".to_string(),
        );

        ProviderConfig {
            key: "test-vertexai".to_string(),
            r#type: "vertexai".to_string(),
            api_key: "".to_string(),
            params,
        }
    }

    fn create_test_chat_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gemini-pro".to_string(),
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(
                    "What's the weather in London?".to_string(),
                )),
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
        }
    }

    #[test]
    fn test_chat_request_conversion() {
        let chat_request = ChatCompletionRequest {
            model: "gemini-pro".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::String("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                },
                ChatCompletionMessage {
                    role: "assistant".to_string(),
                    content: Some(ChatMessageContent::String("Hi there!".to_string())),
                    name: None,
                    tool_calls: None,
                },
            ],
            temperature: Some(0.7),
            top_p: Some(0.9),
            n: Some(1),
            stream: None,
            max_tokens: Some(100),
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tool_choice: None,
            tools: None,
            stop: None,
            parallel_tool_calls: None,
        };

        let vertex_request: VertexAIChatCompletionRequest = chat_request.into();

        assert_eq!(vertex_request.contents.len(), 2);
        assert_eq!(vertex_request.contents[0].role, "user");
        assert_eq!(
            vertex_request.contents[0].parts[0].text,
            Some("Hello".to_string())
        );
        assert_eq!(vertex_request.contents[1].role, "model");
        assert_eq!(
            vertex_request.contents[1].parts[0].text,
            Some("Hi there!".to_string())
        );

        let gen_config = vertex_request.generation_config.unwrap();
        assert_eq!(gen_config.temperature, Some(0.7));
        assert_eq!(gen_config.top_p, Some(0.9));
        assert_eq!(gen_config.max_output_tokens, Some(100));
    }

    #[test]
    fn test_chat_response_conversion() {
        let vertex_response = VertexAIChatCompletionResponse {
            candidates: vec![GenerateContentResponse {
                content: Content {
                    role: "model".to_string(),
                    parts: vec![Part {
                        text: Some("Generated response".to_string()),
                        function_call: None,
                    }],
                },
                finish_reason: "stop".to_string(),
                safety_ratings: None,
                avg_logprobs: None,
                function_call: None,
            }],
            usage_metadata: Some(UsageMetadata {
                prompt_token_count: 10,
                candidates_token_count: 20,
                total_token_count: 30,
            }),
            model_version: Some("v1".to_string()),
        };

        let chat_completion: ChatCompletion = vertex_response.into();

        assert_eq!(chat_completion.choices.len(), 1);
        assert_eq!(chat_completion.choices[0].index, 0);
        assert_eq!(
            chat_completion.choices[0].finish_reason,
            Some("stop".to_string())
        );

        if let Some(ChatMessageContent::String(content)) =
            &chat_completion.choices[0].message.content
        {
            assert_eq!(content, "Generated response");
        } else {
            panic!("Expected String content");
        }
    }

    #[test]
    fn test_embeddings_request_conversion() {
        let embeddings_request = EmbeddingsRequest {
            model: "textembedding-gecko".to_string(),
            input: EmbeddingsInput::Multiple(vec![
                "First text".to_string(),
                "Second text".to_string(),
            ]),
            user: None,
            encoding_format: None,
        };

        let vertex_request: VertexAIEmbeddingsRequest = embeddings_request.into();

        assert_eq!(vertex_request.instances.len(), 2);
        assert_eq!(vertex_request.instances[0].content, "First text");
        assert_eq!(vertex_request.instances[1].content, "Second text");
        assert!(vertex_request.parameters.unwrap().auto_truncate.unwrap());
    }

    #[test]
    fn test_streaming_endpoint_selection() {
        let config = create_test_config();
        let provider = VertexAIProvider::new(&config);

        let mut request = create_test_chat_request();
        request.stream = Some(true);

        let endpoint = provider.get_endpoint(&request);
        assert_eq!(endpoint, "streamGenerateContent");

        request.stream = Some(false);
        let endpoint = provider.get_endpoint(&request);
        assert_eq!(endpoint, "generateContent");
    }

    #[test]
    fn test_provider_initialization() {
        let config = create_test_config();
        let provider = VertexAIProvider::new(&config);

        assert_eq!(provider.key(), "test-vertexai");
        assert_eq!(provider.r#type(), "vertexai");
    }

    #[test]
    fn test_url_construction() {
        let config = create_test_config();
        let provider = VertexAIProvider::new(&config);
        let chat_url = provider.construct_url(
            "gemini-pro",
            "generateContent",
            "test-project",
            "us-central1",
        );

        assert_eq!(
            chat_url,
            "https://us-central1-aiplatform.googleapis.com/v1/projects/test-project/locations/us-central1/publishers/google/models/gemini-pro:generateContent"
        );
    }

    #[tokio::test]
    async fn test_header_construction() {
        let config = create_test_config();
        let provider = VertexAIProvider::new(&config);

        let result = provider.create_headers("test-token").await;

        assert!(result.is_ok());
        let headers = result.unwrap();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer test-token");
        assert_eq!(headers.get("Content-Type").unwrap(), "application/json");
    }
    #[test]
    fn test_embeddings_request_construction() {
        let request = EmbeddingsRequest {
            model: "textembedding-gecko".to_string(),
            input: EmbeddingsInput::Single("test text".to_string()),
            user: None,
            encoding_format: None,
        };

        let vertex_request: VertexAIEmbeddingsRequest = request.into();
        assert_eq!(vertex_request.instances[0].content, "test text");
        assert!(vertex_request.parameters.unwrap().auto_truncate.unwrap());
    }

    #[test]
    fn test_function_calling_request() {
        let mut chat_request = create_test_chat_request();
        chat_request.tools = Some(vec![ToolDefinition {
            function: FunctionDefinition {
                name: "get_weather".to_string(),
                description: Some("Get the current weather in a location".to_string()),
                parameters: Some(HashMap::from([
                    ("type".to_string(), json!("object")),
                    (
                        "properties".to_string(),
                        json!({
                            "location": {
                                "type": "string",
                                "description": "The city name"
                            }
                        }),
                    ),
                    ("required".to_string(), json!(["location"])),
                ])),
                strict: None,
            },
            tool_type: "function".to_string(),
        }]);

        let vertex_request: VertexAIChatCompletionRequest = chat_request.into();

        assert!(!vertex_request.tools.is_empty());
        assert_eq!(
            vertex_request.tools[0].function_declarations[0].name,
            "get_weather"
        );
        assert_eq!(
            vertex_request.tools[0].function_declarations[0].description,
            Some("Get the current weather in a location".to_string())
        );
    }

    #[test]
    fn test_function_calling_response() {
        let vertex_response = VertexAIChatCompletionResponse {
            candidates: vec![GenerateContentResponse {
                content: Content {
                    role: "model".to_string(),
                    parts: vec![Part {
                        text: None,
                        function_call: Some(VertexFunctionCall {
                            name: "get_weather".to_string(),
                            args: json!({"location": "London"}),
                        }),
                    }],
                },
                finish_reason: "stop".to_string(),
                safety_ratings: None,
                avg_logprobs: None,
                function_call: None,
            }],
            usage_metadata: None,
            model_version: None,
        };

        let chat_completion: ChatCompletion = vertex_response.into();

        let tool_calls = chat_completion.choices[0]
            .message
            .tool_calls
            .as_ref()
            .unwrap();
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert_eq!(tool_calls[0].function.arguments, r#"{"location":"London"}"#);
    }
}
