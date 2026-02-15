use std::collections::HashMap;

use async_trait::async_trait;
use hub_lib::guardrails::types::GuardrailClient;
use hub_lib::guardrails::types::{
    EvaluatorResponse, Guard, GuardMode, GuardrailError, OnFailure,
};
use hub_lib::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use hub_lib::models::content::{ChatCompletionMessage, ChatMessageContent};
use hub_lib::models::usage::Usage;
use serde_json::json;

// ---------------------------------------------------------------------------
// Guard config builders
// ---------------------------------------------------------------------------

pub fn create_test_guard(name: &str, mode: GuardMode) -> Guard {
    Guard {
        name: name.to_string(),
        provider: "traceloop".to_string(),
        evaluator_slug: "pii-detector".to_string(),
        params: HashMap::new(),
        mode,
        on_failure: OnFailure::Block,
        required: true,
        api_base: Some("http://localhost:8080".to_string()),
        api_key: Some("test-api-key".to_string()),
    }
}

pub fn create_test_guard_with_failure_action(
    name: &str,
    mode: GuardMode,
    on_failure: OnFailure,
) -> Guard {
    let mut guard = create_test_guard(name, mode);
    guard.on_failure = on_failure;
    guard
}

pub fn create_test_guard_with_required(name: &str, mode: GuardMode, required: bool) -> Guard {
    let mut guard = create_test_guard(name, mode);
    guard.required = required;
    guard
}

#[allow(dead_code)]
pub fn create_test_guard_with_api_base(name: &str, mode: GuardMode, api_base: &str) -> Guard {
    let mut guard = create_test_guard(name, mode);
    guard.api_base = Some(api_base.to_string());
    guard
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

#[allow(dead_code)]
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
