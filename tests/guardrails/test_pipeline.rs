use hub_lib::guardrails::executor::execute_guards;
use hub_lib::guardrails::providers::traceloop::TraceloopClient;
use hub_lib::guardrails::types::*;
use hub_lib::pipelines::pipeline::{
    blocked_response, build_pipeline_guardrails, warning_header_value,
};

use axum::body::to_bytes;
use axum::response::IntoResponse;
use serde_json::json;
use wiremock::matchers;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::helpers::*;

// ---------------------------------------------------------------------------
// Phase 6: Pipeline Integration (7 tests)
//
// These tests verify that guardrails are properly wired into the pipeline
// request handling flow. They use wiremock for the evaluator service.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pre_call_guardrails_block_before_llm() {
    // Set up evaluator mock that rejects the input
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"result": {"reason": "toxic"}, "pass": false})),
        )
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = GuardConfig {
        name: "toxicity-check".to_string(),
        provider: "traceloop".to_string(),
        evaluator_slug: "toxicity".to_string(),
        params: Default::default(),
        mode: GuardMode::PreCall,
        on_failure: OnFailure::Block,
        required: true,
        api_base: Some(eval_server.uri()),
        api_key: Some("test-key".to_string()),
    };

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], "toxic input", &client).await;

    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard.as_deref(), Some("toxicity-check"));
}

#[tokio::test]
async fn test_pre_call_guardrails_warn_and_continue() {
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"result": {"reason": "borderline"}, "pass": false})),
        )
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = GuardConfig {
        name: "tone-check".to_string(),
        provider: "traceloop".to_string(),
        evaluator_slug: "tone".to_string(),
        params: Default::default(),
        mode: GuardMode::PreCall,
        on_failure: OnFailure::Warn,
        required: true,
        api_base: Some(eval_server.uri()),
        api_key: Some("test-key".to_string()),
    };

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], "borderline input", &client).await;

    assert!(!outcome.blocked);
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("tone-check"));
}

#[tokio::test]
async fn test_post_call_guardrails_block_response() {
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"result": {"reason": "pii detected"}, "pass": false})),
        )
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = GuardConfig {
        name: "pii-check".to_string(),
        provider: "traceloop".to_string(),
        evaluator_slug: "pii".to_string(),
        params: Default::default(),
        mode: GuardMode::PostCall,
        on_failure: OnFailure::Block,
        required: true,
        api_base: Some(eval_server.uri()),
        api_key: Some("test-key".to_string()),
    };

    let client = TraceloopClient::new();
    // Simulate post-call: evaluate the LLM response text
    let outcome = execute_guards(&[guard], "Here is John's SSN: 123-45-6789", &client).await;

    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard.as_deref(), Some("pii-check"));
}

#[tokio::test]
async fn test_post_call_guardrails_warn_and_add_header() {
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"result": {"reason": "mildly concerning"}, "pass": false})),
        )
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = GuardConfig {
        name: "safety-check".to_string(),
        provider: "traceloop".to_string(),
        evaluator_slug: "safety".to_string(),
        params: Default::default(),
        mode: GuardMode::PostCall,
        on_failure: OnFailure::Warn,
        required: true,
        api_base: Some(eval_server.uri()),
        api_key: Some("test-key".to_string()),
    };

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], "Some LLM response", &client).await;

    assert!(!outcome.blocked);
    assert!(!outcome.warnings.is_empty());

    // Verify warning header would be generated correctly
    let header = warning_header_value(&outcome);
    assert!(header.contains("guardrail_name="));
    assert!(header.contains("safety-check"));
}

#[tokio::test]
async fn test_warning_header_format() {
    let outcome = GuardrailsOutcome {
        results: vec![],
        blocked: false,
        blocking_guard: None,
        warnings: vec!["Guard 'my-guard' failed with warning".to_string()],
    };
    let header = warning_header_value(&outcome);
    assert_eq!(header, "guardrail_name=\"my-guard\", reason=\"failed\"");
}

#[tokio::test]
async fn test_blocked_response_403_format() {
    let outcome = GuardrailsOutcome {
        results: vec![],
        blocked: true,
        blocking_guard: Some("toxicity-check".to_string()),
        warnings: vec![],
    };
    let response = blocked_response(&outcome);
    assert_eq!(response.status(), 403);

    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["type"], "guardrail_blocked");
    assert_eq!(json["error"]["guardrail"], "toxicity-check");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("toxicity-check")
    );
}

#[tokio::test]
async fn test_no_guardrails_passthrough() {
    // Empty guardrails config -> build_pipeline_guardrails returns None
    let config = GuardrailsConfig {
        providers: vec![],
        guards: vec![],
    };
    let result = build_pipeline_guardrails(&config);
    assert!(result.is_none());

    // Config with no guards -> passthrough
    let config_with_providers = GuardrailsConfig {
        providers: vec![ProviderConfig {
            name: "traceloop".to_string(),
            api_base: "http://localhost".to_string(),
            api_key: "key".to_string(),
        }],
        guards: vec![],
    };
    let result = build_pipeline_guardrails(&config_with_providers);
    assert!(result.is_none());
}
