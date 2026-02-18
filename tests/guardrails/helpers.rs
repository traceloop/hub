use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use hub_lib::guardrails::types::GuardrailClient;
use hub_lib::guardrails::types::{EvaluatorResponse, Guard, GuardMode, GuardrailError, OnFailure};
use hub_lib::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use hub_lib::models::completion::{CompletionChoice, CompletionRequest, CompletionResponse};
use hub_lib::models::content::{ChatCompletionMessage, ChatMessageContent};
use hub_lib::models::embeddings::{
    Embedding, Embeddings, EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse,
};
use hub_lib::models::usage::{EmbeddingUsage, Usage};
use serde::Serialize;
use serde_json::json;
use tower::Service;

// ---------------------------------------------------------------------------
// Guard config builders
// ---------------------------------------------------------------------------

pub struct TestGuardBuilder {
    guard: Guard,
}

impl TestGuardBuilder {
    pub fn new(name: &str, mode: GuardMode) -> Self {
        Self {
            guard: Guard {
                name: name.to_string(),
                provider: "traceloop".to_string(),
                evaluator_slug: "pii-detector".to_string(),
                params: HashMap::new(),
                mode,
                on_failure: OnFailure::Block,
                required: false,
                api_base: Some("http://localhost:8080".to_string()),
                api_key: Some("test-api-key".to_string()),
            },
        }
    }

    pub fn on_failure(mut self, on_failure: OnFailure) -> Self {
        self.guard.on_failure = on_failure;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.guard.required = required;
        self
    }

    pub fn api_base(mut self, api_base: &str) -> Self {
        self.guard.api_base = Some(api_base.to_string());
        self
    }

    pub fn evaluator_slug(mut self, slug: &str) -> Self {
        self.guard.evaluator_slug = slug.to_string();
        self
    }

    pub fn build(self) -> Guard {
        self.guard
    }
}

// Backward-compatible helper functions
pub fn create_test_guard(name: &str, mode: GuardMode) -> Guard {
    TestGuardBuilder::new(name, mode).build()
}

pub fn create_test_guard_with_failure_action(
    name: &str,
    mode: GuardMode,
    on_failure: OnFailure,
) -> Guard {
    TestGuardBuilder::new(name, mode)
        .on_failure(on_failure)
        .build()
}

pub fn create_test_guard_with_required(name: &str, mode: GuardMode, required: bool) -> Guard {
    TestGuardBuilder::new(name, mode).required(required).build()
}

pub fn create_test_guard_with_api_base(name: &str, mode: GuardMode, api_base: &str) -> Guard {
    TestGuardBuilder::new(name, mode).api_base(api_base).build()
}

// ---------------------------------------------------------------------------
// Evaluator response builders
// ---------------------------------------------------------------------------

pub fn passing_response() -> EvaluatorResponse {
    EvaluatorResponse {
        result: json!({"score": 0.95, "label": "safe"}),
        pass: true,
    }
}

pub fn failing_response() -> EvaluatorResponse {
    EvaluatorResponse {
        result: json!({"score": 0.2, "label": "unsafe", "reason": "Content violates policy"}),
        pass: false,
    }
}

// ---------------------------------------------------------------------------
// Chat request/response builders
// ---------------------------------------------------------------------------

pub fn default_message() -> ChatCompletionMessage {
    ChatCompletionMessage {
        role: String::new(),
        content: None,
        name: None,
        tool_calls: None,
        tool_call_id: None,
        refusal: None,
    }
}

pub fn default_request() -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
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
    }
}

pub fn create_test_chat_request(user_message: &str) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(user_message.to_string())),
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
    }
}

pub fn create_test_chat_completion(response_text: &str) -> ChatCompletion {
    ChatCompletion {
        id: "chatcmpl-test".to_string(),
        object: Some("chat.completion".to_string()),
        created: Some(1234567890),
        model: "gpt-4".to_string(),
        choices: vec![ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: "assistant".to_string(),
                content: Some(ChatMessageContent::String(response_text.to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        }],
        usage: Usage::default(),
        system_fingerprint: None,
    }
}

// ---------------------------------------------------------------------------
// Completion request/response builders
// ---------------------------------------------------------------------------

pub fn create_test_completion_request(prompt: &str) -> CompletionRequest {
    CompletionRequest {
        model: "gpt-3.5-turbo-instruct".to_string(),
        prompt: prompt.to_string(),
        suffix: None,
        max_tokens: Some(100),
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
    }
}

pub fn create_test_completion_response(text: &str) -> CompletionResponse {
    CompletionResponse {
        id: "cmpl-test".to_string(),
        object: "text_completion".to_string(),
        created: 1234567890,
        model: "gpt-3.5-turbo-instruct".to_string(),
        choices: vec![CompletionChoice {
            text: text.to_string(),
            index: 0,
            logprobs: None,
            finish_reason: Some("stop".to_string()),
        }],
        usage: Usage::default(),
    }
}

// ---------------------------------------------------------------------------
// Embeddings request/response builders
// ---------------------------------------------------------------------------

pub fn create_test_embeddings_request(text: &str) -> EmbeddingsRequest {
    EmbeddingsRequest {
        model: "text-embedding-ada-002".to_string(),
        input: EmbeddingsInput::Single(text.to_string()),
        user: None,
        encoding_format: None,
    }
}

pub fn create_test_embeddings_response() -> EmbeddingsResponse {
    EmbeddingsResponse {
        object: "list".to_string(),
        data: vec![Embeddings {
            object: "embedding".to_string(),
            embedding: Embedding::Float(vec![0.1, 0.2, 0.3]),
            index: 0,
        }],
        model: "text-embedding-ada-002".to_string(),
        usage: EmbeddingUsage {
            prompt_tokens: Some(8),
            total_tokens: Some(8),
        },
    }
}

// ---------------------------------------------------------------------------
// Streaming request builders
// ---------------------------------------------------------------------------

pub fn create_streaming_chat_request(message: &str) -> ChatCompletionRequest {
    let mut req = create_test_chat_request(message);
    req.stream = Some(true);
    req
}

pub fn create_streaming_completion_request(prompt: &str) -> CompletionRequest {
    let mut req = create_test_completion_request(prompt);
    req.stream = Some(true);
    req
}

// ---------------------------------------------------------------------------
// Mock Service for middleware testing
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MockService {
    status: StatusCode,
    body: Vec<u8>,
}

impl MockService {
    pub fn with_json<T: Serialize>(status: StatusCode, data: &T) -> Self {
        let body = serde_json::to_vec(data).unwrap();
        Self { status, body }
    }
}

impl Service<Request<Body>> for MockService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<Body>) -> Self::Future {
        let status = self.status;
        let body = self.body.clone();
        Box::pin(async move {
            let response = Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap();
            Ok(response)
        })
    }
}

// ---------------------------------------------------------------------------
// Mock GuardrailClient
// ---------------------------------------------------------------------------

pub struct MockGuardrailClient {
    pub responses: HashMap<String, Result<EvaluatorResponse, GuardrailError>>,
}

impl MockGuardrailClient {
    pub fn with_response(name: &str, resp: Result<EvaluatorResponse, GuardrailError>) -> Self {
        let mut responses = HashMap::new();
        responses.insert(name.to_string(), resp);
        Self { responses }
    }

    pub fn with_responses(entries: Vec<(&str, Result<EvaluatorResponse, GuardrailError>)>) -> Self {
        let mut responses = HashMap::new();
        for (name, resp) in entries {
            responses.insert(name.to_string(), resp);
        }
        Self { responses }
    }
}

#[async_trait]
impl GuardrailClient for MockGuardrailClient {
    async fn evaluate(
        &self,
        guard: &Guard,
        _input: &str,
    ) -> Result<EvaluatorResponse, GuardrailError> {
        self.responses
            .get(&guard.name)
            .cloned()
            .unwrap_or(Err(GuardrailError::Unavailable(
                "no mock configured".to_string(),
            )))
    }
}
