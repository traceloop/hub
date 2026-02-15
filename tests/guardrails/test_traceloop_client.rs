use hub_lib::guardrails::providers::create_guardrail_client;
use hub_lib::guardrails::providers::traceloop::TraceloopClient;
use hub_lib::guardrails::types::GuardMode;
use hub_lib::guardrails::types::GuardrailClient;
use serde_json::json;
use wiremock::matchers;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::helpers::*;

// ---------------------------------------------------------------------------
// Phase 4: Provider Client System (7 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_traceloop_client_constructs_correct_url() {
    let mock_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v2/guardrails/execute/toxicity-detector"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"result": {}, "pass": true})))
        .expect(1)
        .mount(&mock_server)
        .await;

    let mut guard = create_test_guard_with_api_base("test", GuardMode::PreCall, &mock_server.uri());
    guard.evaluator_slug = "toxicity-detector".to_string();

    let client = TraceloopClient::new();
    let result = client.evaluate(&guard, "test input").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_traceloop_client_sends_correct_headers() {
    let mock_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::header("Authorization", "Bearer test-api-key"))
        .and(matchers::header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"result": {}, "pass": true})))
        .expect(1)
        .mount(&mock_server)
        .await;

    let guard = create_test_guard_with_api_base("test", GuardMode::PreCall, &mock_server.uri());
    let client = TraceloopClient::new();
    let result = client.evaluate(&guard, "test input").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_traceloop_client_sends_correct_body() {
    let mock_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::body_json(json!({
            "input": {"text": "test input text"},
            "config": {"threshold": 0.5}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"result": {}, "pass": true})))
        .expect(1)
        .mount(&mock_server)
        .await;

    let mut guard = create_test_guard_with_api_base("test", GuardMode::PreCall, &mock_server.uri());
    guard.params.insert("threshold".to_string(), json!(0.5));

    let client = TraceloopClient::new();
    let result = client.evaluate(&guard, "test input text").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_traceloop_client_handles_successful_response() {
    let mock_server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"score": 0.9, "label": "safe"},
            "pass": true
        })))
        .mount(&mock_server)
        .await;

    let guard = create_test_guard_with_api_base("test", GuardMode::PreCall, &mock_server.uri());
    let client = TraceloopClient::new();
    let result = client.evaluate(&guard, "safe input").await.unwrap();
    assert!(result.pass);
    assert_eq!(result.result["score"], 0.9);
}

#[tokio::test]
async fn test_traceloop_client_handles_error_response() {
    let mock_server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let guard = create_test_guard_with_api_base("test", GuardMode::PreCall, &mock_server.uri());
    let client = TraceloopClient::new();
    let result = client.evaluate(&guard, "test").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_traceloop_client_handles_timeout() {
    let mock_server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"result": {}, "pass": true}))
                .set_delay(std::time::Duration::from_secs(30)),
        )
        .mount(&mock_server)
        .await;

    let guard = create_test_guard_with_api_base("test", GuardMode::PreCall, &mock_server.uri());
    let client = TraceloopClient::with_timeout(std::time::Duration::from_millis(100));
    let result = client.evaluate(&guard, "test").await;
    assert!(result.is_err());
}

#[test]
fn test_client_creation_from_guard_config() {
    let guard = create_test_guard("test", GuardMode::PreCall);
    let client = create_guardrail_client(&guard);
    assert!(client.is_some());
}
