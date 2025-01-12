use hub::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use hub::models::content::{ChatCompletionMessage, ChatMessageContent};
use hub::models::embeddings::{EmbeddingsInput, EmbeddingsRequest};
use hub::models::tool_definition::{FunctionDefinition, ToolDefinition};
use hub::providers::provider::Provider;
use hub::providers::vertexai::VertexAIProvider;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const RECORDINGS_DIR: &str = "tests/recordings/vertexai";

#[derive(serde::Serialize, serde::Deserialize)]
struct RecordedInteraction {
    request: RequestData,
    response: ResponseData,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RequestData {
    method: wiremock::http::Method,
    path: String,
    headers: HashMap<String, String>,
    body: Option<Value>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ResponseData {
    status: u16,
    body: Value,
}

struct TestConfig {
    project_id: String,
    location: String,
}

impl TestConfig {
    fn new() -> Self {
        if env::var("RECORD").is_ok() {
            dotenv::from_filename(".env").ok();
            Self {
                project_id: env::var("VERTEX_PROJECT_ID")
                    .expect("VERTEX_PROJECT_ID must be set for recording"),
                location: env::var("VERTEX_LOCATION")
                    .unwrap_or_else(|_| "us-central1".to_string()),
            }
        } else {
            Self::from_recordings().unwrap_or_else(|| Self {
                project_id: "extended-legend-445620-u1".to_string(), 
                location: "us-central1".to_string(),
            })
        }
    }

    fn from_recordings() -> Option<Self> {
        let recordings_dir = PathBuf::from(RECORDINGS_DIR);
        if let Ok(content) = fs::read_to_string(recordings_dir.join("chat_completion.json")) {
            if let Ok(interactions) = serde_json::from_str::<Vec<RecordedInteraction>>(&content) {
                if let Some(interaction) = interactions.first() {
                    if let Some((project_id, location)) = Self::extract_from_path(&interaction.request.path) {
                        return Some(Self {
                            project_id: project_id.to_string(),
                            location: location.to_string(),
                        });
                    }
                }
            }
        }
        None
    }

    fn extract_from_path(path: &str) -> Option<(&str, &str)> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 7 {
            Some((parts[3], parts[5]))
        } else {
            None
        }
    }
}

async fn setup_mock_server(test_name: &str) -> MockServer {
    let mock_server = MockServer::start().await;
    let recordings_dir = PathBuf::from(RECORDINGS_DIR);
    
    if env::var("RECORD").is_err() {
        if let Ok(content) = fs::read_to_string(recordings_dir.join(format!("{}.json", test_name))) {
            if let Ok(interactions) = serde_json::from_str::<Vec<RecordedInteraction>>(&content) {
                for interaction in interactions {
                    setup_mock(&mock_server, interaction).await;
                }
            }
        }
    }
    
    mock_server
}

async fn setup_mock(mock_server: &MockServer, interaction: RecordedInteraction) {
    let mut mock = Mock::given(method(interaction.request.method))
        .and(path(&interaction.request.path));

    for (key, value) in interaction.request.headers {
        mock = mock.and(wiremock::matchers::header(key.as_str(), value.as_str()));
    }

    mock.respond_with(ResponseTemplate::new(interaction.response.status)
        .set_body_json(interaction.response.body))
        .mount(mock_server)
        .await;
}

async fn save_interaction(test_name: &str, interaction: RecordedInteraction) {
    let recordings_dir = PathBuf::from(RECORDINGS_DIR);
    fs::create_dir_all(&recordings_dir).unwrap_or_default();
    let recording_path = recordings_dir.join(format!("{}.json", test_name));

    let mut interactions = Vec::new();
    if let Ok(content) = fs::read_to_string(&recording_path) {
        if let Ok(mut existing) = serde_json::from_str::<Vec<RecordedInteraction>>(&content) {
            interactions.append(&mut existing);
        }
    }

    interactions.push(interaction);
    
    if let Ok(content) = serde_json::to_string_pretty(&interactions) {
        fs::write(&recording_path, content).expect("Failed to save recording");
    }
}

async fn create_test_provider(mock_server: &MockServer) -> VertexAIProvider {
    let config = TestConfig::new();
    let mut params = HashMap::new();
    params.insert("project_id".to_string(), config.project_id);
    params.insert("location".to_string(), config.location);
    
    if env::var("RECORD").is_err() {
        params.insert("test_base_url".to_string(), mock_server.uri());
        params.insert("skip_authentication".to_string(), "true".to_string());
    }

    let provider_config = hub::config::models::Provider {
        key: "vertexai".to_string(),
        r#type: "vertexai".to_string(),
        api_key: String::new(),
        params,
    };

    VertexAIProvider::new(&provider_config)
}

async fn record_chat_response(response: &ChatCompletionResponse) -> Value {
    match response {
        ChatCompletionResponse::NonStream(completion) => {
            serde_json::to_value(completion).unwrap_or_else(|_| json!(null))
        }
        ChatCompletionResponse::Stream(_) => {
            json!({
                "type": "stream",
                "status": "recorded"
            })
        }
    }
}

#[tokio::test]
async fn test_chat_completion() {
    let mock_server = setup_mock_server("chat_completion").await;
    let provider = create_test_provider(&mock_server).await;
    let config = TestConfig::new();
    
    let model_config = hub::config::models::ModelConfig {
        key: "gemini-pro".to_string(),
        r#type: "gemini-pro".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

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

    let response = provider
        .chat_completions(request.clone(), &model_config)
        .await;

    if env::var("RECORD").is_ok() {
        if let Ok(resp) = &response {
            let response_value = match resp {
                ChatCompletionResponse::NonStream(completion) => {
                    serde_json::to_value(completion).unwrap()
                }
                ChatCompletionResponse::Stream(_) => {
                    json!({
                        "type": "stream",
                        "status": "recorded"
                    })
                }
            };
            
            let interaction = RecordedInteraction {
                request: RequestData {
                    method: wiremock::http::Method::Post,
                    path: format!(
                        "/v1/projects/{}/locations/{}/publishers/google/models/gemini-pro:generateContent",
                        config.project_id, config.location
                    ),
                    headers: HashMap::new(),
                    body: Some(serde_json::to_value(&request).unwrap()),
                },
                response: ResponseData {
                    status: 200,
                    body: response_value,
                },
            };
            save_interaction("chat_completion", interaction).await;
        }
    }

    assert!(response.is_ok(), "Chat completion request failed");
    if let Ok(ChatCompletionResponse::NonStream(completion)) = response {
        assert!(!completion.choices.is_empty(), "No choices in response");
        assert!(completion.choices[0].message.content.is_some(), "No content in response");
    }
}

#[tokio::test]
async fn test_chat_completion_with_tools() {
    let mock_server = setup_mock_server("chat_completion_with_tools").await;
    let provider = create_test_provider(&mock_server).await;
    
    let model_config = hub::config::models::ModelConfig {
        key: "gemini-pro".to_string(),
        r#type: "gemini-pro".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let request = ChatCompletionRequest {
        model: "gemini-pro".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(
                "What's the weather in San Francisco?".to_string(),
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
        user: None,
        parallel_tool_calls: None,
    };

    let response = provider
        .chat_completions(request.clone(), &model_config)
        .await;

    if env::var("RECORD").is_ok() {
        if let Ok(resp) = &response {
            let interaction = RecordedInteraction {
                request: RequestData {
                    method: wiremock::http::Method::Post,
                    path: "gemini-pro:generateContent".to_string(),
                    headers: HashMap::new(),
                    body: Some(serde_json::to_value(&request).unwrap()),
                },
                response: ResponseData {
                    status: 200,
                    body: record_chat_response(resp).await,
                },
            };
            save_interaction("chat_completion_with_tools", interaction).await;
        }
    }

    assert!(
        response.is_ok(),
        "Chat completion with tools request failed"
    );

    if let Ok(ChatCompletionResponse::NonStream(completion)) = response {
        assert!(!completion.choices.is_empty(), "No choices in response");
        let tool_calls = completion.choices[0].message.tool_calls.as_ref();
        assert!(tool_calls.is_some(), "No tool calls in response");

        if let Some(tool_calls) = tool_calls {
            assert!(!tool_calls.is_empty(), "Empty tool calls");
            assert_eq!(
                tool_calls[0].function.name, "get_weather",
                "Incorrect function name"
            );

            let args: Value = serde_json::from_str(&tool_calls[0].function.arguments).unwrap();
            assert!(args["location"].is_string(), "Location should be a string");
        }
    }
}

#[tokio::test]
async fn test_embeddings_functionality() {
    println!("Starting embeddings test");
    let mock_server = setup_mock_server("embeddings_test").await;
    let provider = create_test_provider(&mock_server).await;

    let model_config = hub::config::models::ModelConfig {
        key: "textembedding-gecko".to_string(),
        r#type: "textembedding-gecko".to_string(),
        provider: "vertexai".to_string(),
        params: HashMap::new(),
    };

    let request = EmbeddingsRequest {
        model: "textembedding-gecko".to_string(),
        input: EmbeddingsInput::Single("Explore embeddings test.".to_string()),
        user: None,
        encoding_format: None,
    };

    let response = provider.embeddings(request.clone(), &model_config).await;

    if env::var("RECORD").is_ok() {
        if let Ok(resp) = &response {
            let config = TestConfig::new();
            let interaction = RecordedInteraction {
                request: RequestData {
                    method: wiremock::http::Method::Post,
                    path: format!(
                        "/v1/projects/{}/locations/{}/publishers/google/models/textembedding-gecko:predict",
                        config.project_id, config.location
                    ),
                    headers: HashMap::new(),
                    body: Some(serde_json::to_value(&request).unwrap()),
                },
                response: ResponseData {
                    status: 200,
                    body: serde_json::to_value(resp).unwrap(),
                },
            };
            save_interaction("embeddings_test", interaction).await;
        }
    }

    assert!(response.is_ok(), "Embeddings request failed");

    if let Ok(embeddings) = response {
        assert!(!embeddings.data.is_empty(), "Embeddings response is empty");
        assert!(
            !embeddings.data[0].embedding.is_empty(),
            "Embedding vector is empty"
        );
    }
}
