use hub_lib::guardrails::middleware::GuardrailsLayer;
use hub_lib::guardrails::providers::traceloop::TraceloopClient;
use hub_lib::guardrails::types::{Guard, GuardMode, Guardrails, OnFailure};

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::StatusCode;
use serde_json::json;
use std::sync::Arc;
use tower::{Layer, Service, ServiceExt};
use wiremock::matchers;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::helpers::*;

/// Helper to create a guard with a specific wiremock server
fn guard_with_server(
    name: &str,
    mode: GuardMode,
    on_failure: OnFailure,
    server_uri: &str,
) -> Guard {
    Guard {
        name: name.to_string(),
        provider: "traceloop".to_string(),
        evaluator_slug: "toxicity-detector".to_string(),
        params: Default::default(),
        mode,
        on_failure,
        required: false,
        api_base: Some(server_uri.to_string()),
        api_key: Some("test-key".to_string()),
    }
}

/// Helper to create a complete guardrails configuration
fn create_guardrails(guards: Vec<Guard>) -> Guardrails {
    let guard_names: Vec<String> = guards.iter().map(|g| g.name.clone()).collect();
    Guardrails {
        all_guards: Arc::new(guards),
        pipeline_guard_names: guard_names,
        client: Arc::new(TraceloopClient::new()),
    }
}

// ===========================================================================
// Category 1: Endpoint Type Detection
// ===========================================================================

#[tokio::test]
async fn test_chat_completions_endpoint_detected() {
    // Set up mock evaluator for pre-call guard
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": true
        })))
        .expect(1)
        .mount(&eval_server)
        .await;

    // Create guard
    let guard = guard_with_server(
        "detector",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    // Create mock inner service
    let completion = create_test_chat_completion("Response");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    // Apply middleware
    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    // Create chat request
    let request = create_test_chat_request("Test input");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    // Call middleware
    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify response is 200 OK (guard passed)
    assert_eq!(response.status(), StatusCode::OK);

    // Wiremock verifies evaluator was called (expect(1))
}

#[tokio::test]
async fn test_completions_endpoint_detected() {
    // Set up mock evaluator
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": true
        })))
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "detector",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let completion = create_test_completion_response("Response text");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    // Create completion request
    let request = create_test_completion_request("Complete this");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_embeddings_endpoint_detected() {
    // Set up mock evaluator
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": true
        })))
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "detector",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let embeddings_response = create_test_embeddings_response();
    let inner_service = MockService::with_json(StatusCode::OK, &embeddings_response);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    // Create embeddings request
    let request = create_test_embeddings_request("Embed this text");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ===========================================================================
// Category 2: Pre-Call Guard Behavior
// ===========================================================================

#[tokio::test]
async fn test_pre_call_guard_blocks_chat() {
    // Set up mock evaluator that fails
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"reason": "toxic content"},
            "pass": false
        })))
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "blocker",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let completion = create_test_chat_completion("This won't be returned");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    let request = create_test_chat_request("Bad input");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify blocked
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(response_json["error"]["guardrail"], "blocker");
}

#[tokio::test]
async fn test_pre_call_guard_warns_chat() {
    // Set up mock evaluator that fails with warn
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"reason": "borderline"},
            "pass": false
        })))
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "warner",
        GuardMode::PreCall,
        OnFailure::Warn,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let completion = create_test_chat_completion("Response text");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    let request = create_test_chat_request("Borderline input");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify passes with warning
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .contains_key("x-traceloop-guardrail-warning")
    );

    let warning_header = response
        .headers()
        .get("x-traceloop-guardrail-warning")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(warning_header.contains("warner"));
}

// ===========================================================================
// Category 3: Post-Call Guard Behavior
// ===========================================================================

#[tokio::test]
async fn test_post_call_guard_blocks_chat() {
    // Set up mock evaluator for post-call that fails
    let eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"reason": "unsafe output"},
            "pass": false
        })))
        .expect(1)
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "output-blocker",
        GuardMode::PostCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let completion = create_test_chat_completion("Unsafe response");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    let request = create_test_chat_request("Safe input");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify blocked by post-call guard
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(response_json["error"]["guardrail"], "output-blocker");
}

#[tokio::test]
async fn test_post_call_guard_skipped_for_embeddings() {
    // Set up mock evaluator for pre-call (should be called)
    let pre_eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": true
        })))
        .expect(1) // Pre-call should run
        .mount(&pre_eval_server)
        .await;

    // Set up mock evaluator for post-call (should NOT be called)
    let post_eval_server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": false  // Would block if called
        })))
        .expect(0) // Post-call should NOT run for embeddings
        .mount(&post_eval_server)
        .await;

    // Create both pre-call and post-call guards
    let pre_guard = guard_with_server(
        "pre-guard",
        GuardMode::PreCall,
        OnFailure::Block,
        &pre_eval_server.uri(),
    );
    let post_guard = guard_with_server(
        "post-guard",
        GuardMode::PostCall,
        OnFailure::Block,
        &post_eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![pre_guard, post_guard]);

    let embeddings_response = create_test_embeddings_response();
    let inner_service = MockService::with_json(StatusCode::OK, &embeddings_response);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    let request = create_test_embeddings_request("Embed this");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/embeddings")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify response is 200 OK (no post-call blocking)
    assert_eq!(response.status(), StatusCode::OK);

    // Verify no warning header
    assert!(
        !response
            .headers()
            .contains_key("x-traceloop-guardrail-warning")
    );

    // Verify response body contains embeddings
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(response_json["object"], "list");

    // Wiremock verifies:
    // - pre-call evaluator called exactly once (expect(1))
    // - post-call evaluator never called (expect(0))
}

// ===========================================================================
// Category 4: Streaming Behavior
// ===========================================================================

#[tokio::test]
async fn test_streaming_chat_bypasses_guards() {
    // Set up mock evaluator (should never be called)
    let eval_server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": false  // Would block if evaluated
        })))
        .expect(0) // Should never be called for streaming
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "blocker",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let completion = create_test_chat_completion("Streamed response");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    // Create STREAMING chat request
    let request = create_streaming_chat_request("This would fail guards if checked");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify response is 200 OK (streaming bypasses guards)
    assert_eq!(response.status(), StatusCode::OK);

    // Verify no warning header
    assert!(
        !response
            .headers()
            .contains_key("x-traceloop-guardrail-warning")
    );

    // Wiremock verifies evaluator was never called (expect(0))
}

#[tokio::test]
async fn test_streaming_completion_bypasses_guards() {
    // Set up mock evaluator (should never be called)
    let eval_server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": false  // Would block if evaluated
        })))
        .expect(0) // Should never be called for streaming
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "blocker",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let completion = create_test_completion_response("Streamed completion");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    // Create STREAMING completion request
    let request = create_streaming_completion_request("This would fail guards if checked");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify response is 200 OK (streaming bypasses guards)
    assert_eq!(response.status(), StatusCode::OK);

    // Verify no warning header
    assert!(
        !response
            .headers()
            .contains_key("x-traceloop-guardrail-warning")
    );

    // Wiremock verifies evaluator was never called (expect(0))
}

// ===========================================================================
// Category 5: Pass-Through Scenarios
// ===========================================================================

#[tokio::test]
async fn test_no_guardrails_configured_passes() {
    // No guardrails configured
    let completion = create_test_chat_completion("Response");
    let inner_service = MockService::with_json(StatusCode::OK, &completion);

    let layer = GuardrailsLayer::new(None); // No guardrails
    let mut service = layer.layer(inner_service);

    let request = create_test_chat_request("Any input");
    let http_request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify response passes through unchanged
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_unsupported_endpoint_passes() {
    let eval_server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {},
            "pass": false
        })))
        .expect(0) // Should never be called
        .mount(&eval_server)
        .await;

    let guard = guard_with_server(
        "blocker",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval_server.uri(),
    );
    let guardrails = create_guardrails(vec![guard]);

    let inner_service =
        MockService::with_json(StatusCode::OK, &json!({"data": [{"id": "model-1"}]}));

    let layer = GuardrailsLayer::new(Some(Arc::new(guardrails)));
    let mut service = layer.layer(inner_service);

    // Request to unsupported endpoint
    let http_request = Request::builder()
        .method("GET")
        .uri("/v1/models") // Unsupported endpoint
        .body(Body::empty())
        .unwrap();

    let response = service
        .ready()
        .await
        .unwrap()
        .call(http_request)
        .await
        .unwrap();

    // Verify passes through
    assert_eq!(response.status(), StatusCode::OK);
}
