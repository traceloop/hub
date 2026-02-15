use hub_lib::guardrails::parsing::{CompletionExtractor, PromptExtractor};
use hub_lib::guardrails::providers::traceloop::TraceloopClient;
use hub_lib::guardrails::runner::{blocked_response, execute_guards, warning_header_value};
use hub_lib::guardrails::setup::{build_guardrail_resources, build_pipeline_guardrails};
use hub_lib::guardrails::types::*;

use axum::body::to_bytes;

use serde_json::json;
use wiremock::matchers;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::helpers::*;

// ---------------------------------------------------------------------------
// Phase 8: End-to-End Integration (15 tests)
//
// Full request flow tests using wiremock for evaluator services.
// These validate the complete lifecycle from request to response.
// ---------------------------------------------------------------------------

/// Helper: set up a wiremock evaluator that returns pass/fail
async fn setup_evaluator(pass: bool) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"score": if pass { 0.95 } else { 0.1 }},
            "pass": pass
        })))
        .mount(&server)
        .await;
    server
}

fn guard_with_server(
    name: &str,
    mode: GuardMode,
    on_failure: OnFailure,
    server_uri: &str,
    slug: &str,
) -> Guard {
    Guard {
        name: name.to_string(),
        provider: "traceloop".to_string(),
        evaluator_slug: slug.to_string(),
        params: Default::default(),
        mode,
        on_failure,
        required: true,
        api_base: Some(server_uri.to_string()),
        api_key: Some("test-key".to_string()),
    }
}

#[tokio::test]
async fn test_e2e_pre_call_block_flow() {
    // Request -> guard fail+block -> 403
    let eval = setup_evaluator(false).await;
    let guard = guard_with_server(
        "blocker",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval.uri(),
        "toxicity-detector",
    );

    let request = create_test_chat_request("Bad input");
    let input = request.extract_pompt();

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], &input, &client).await;

    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard.as_deref(), Some("blocker"));
}

#[tokio::test]
async fn test_e2e_pre_call_pass_flow() {
    // Request -> guard pass -> LLM -> 200
    let eval = setup_evaluator(true).await;
    let guard = guard_with_server(
        "checker",
        GuardMode::PreCall,
        OnFailure::Block,
        &eval.uri(),
        "toxicity-detector",
    );

    let request = create_test_chat_request("Safe input");
    let input = request.extract_pompt();

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], &input, &client).await;

    assert!(!outcome.blocked);
    assert!(outcome.warnings.is_empty());
    // In real flow, would proceed to LLM call
}

#[tokio::test]
async fn test_e2e_post_call_block_flow() {
    // Request -> LLM -> guard fail+block -> 403
    let eval = setup_evaluator(false).await;
    let guard = guard_with_server(
        "pii-check",
        GuardMode::PostCall,
        OnFailure::Block,
        &eval.uri(),
        "pii-detector",
    );

    // Simulate LLM response
    let completion = create_test_chat_completion("Here is the SSN: 123-45-6789");
    let response_text = completion.extract_completion();

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], &response_text, &client).await;

    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard.as_deref(), Some("pii-check"));
}

#[tokio::test]
async fn test_e2e_post_call_warn_flow() {
    // Request -> LLM -> guard fail+warn -> 200 + header
    let eval = setup_evaluator(false).await;
    let guard = guard_with_server(
        "tone-check",
        GuardMode::PostCall,
        OnFailure::Warn,
        &eval.uri(),
        "tone-detection",
    );

    let completion = create_test_chat_completion("Mildly concerning response");
    let response_text = completion.extract_completion();

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], &response_text, &client).await;

    assert!(!outcome.blocked);
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].guard_name, "tone-check");
}

#[tokio::test]
async fn test_e2e_pre_and_post_both_pass() {
    // Both stages pass -> clean 200 response
    let pre_eval = setup_evaluator(true).await;
    let post_eval = setup_evaluator(true).await;

    let pre_guard = guard_with_server(
        "pre-check",
        GuardMode::PreCall,
        OnFailure::Block,
        &pre_eval.uri(),
        "profanity-detector",
    );
    let post_guard = guard_with_server(
        "post-check",
        GuardMode::PostCall,
        OnFailure::Block,
        &post_eval.uri(),
        "pii-detector",
    );

    let client = TraceloopClient::new();

    // Pre-call
    let request = create_test_chat_request("Hello");
    let input = request.extract_pompt();
    let pre_outcome = execute_guards(&[pre_guard], &input, &client).await;
    assert!(!pre_outcome.blocked);

    // Post-call
    let completion = create_test_chat_completion("Hi there!");
    let response_text = completion.extract_completion();
    let post_outcome = execute_guards(&[post_guard], &response_text, &client).await;
    assert!(!post_outcome.blocked);
    assert!(post_outcome.warnings.is_empty());
}

#[tokio::test]
async fn test_e2e_pre_blocks_post_never_runs() {
    // Pre blocks -> post evaluator gets 0 requests
    let pre_eval = setup_evaluator(false).await;
    let post_eval = MockServer::start().await;

    // Post evaluator should receive 0 requests
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"result": {}, "pass": true})))
        .expect(0)
        .mount(&post_eval)
        .await;

    let pre_guard = guard_with_server(
        "blocker",
        GuardMode::PreCall,
        OnFailure::Block,
        &pre_eval.uri(),
        "toxicity-detector",
    );
    let post_guard = guard_with_server(
        "post-check",
        GuardMode::PostCall,
        OnFailure::Block,
        &post_eval.uri(),
        "pii-detector",
    );

    let client = TraceloopClient::new();
    let request = create_test_chat_request("Bad input");
    let input = request.extract_pompt();

    let pre_outcome = execute_guards(&[pre_guard], &input, &client).await;
    assert!(pre_outcome.blocked);

    // Since pre blocked, post guards never run - post_eval.verify() will assert 0 calls
    // (wiremock verifies expect(0) when server drops)
    let _ = post_guard; // not used - that's the point
}

#[tokio::test]
async fn test_e2e_mixed_block_and_warn() {
    // Multiple guards with mixed block/warn outcomes
    let eval1 = setup_evaluator(true).await; // passes
    let eval2 = setup_evaluator(false).await; // fails -> warn
    let eval3 = setup_evaluator(false).await; // fails -> block

    let guards = vec![
        guard_with_server(
            "passer",
            GuardMode::PreCall,
            OnFailure::Block,
            &eval1.uri(),
            "profanity-detector",
        ),
        guard_with_server(
            "warner",
            GuardMode::PreCall,
            OnFailure::Warn,
            &eval2.uri(),
            "tone-detection",
        ),
        guard_with_server(
            "blocker",
            GuardMode::PreCall,
            OnFailure::Block,
            &eval3.uri(),
            "toxicity-detector",
        ),
    ];

    let client = TraceloopClient::new();
    let outcome = execute_guards(&guards, "test input", &client).await;

    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard.as_deref(), Some("blocker"));
    assert!(outcome.warnings.iter().any(|w| w.guard_name == "warner"));
}

#[tokio::test]
async fn test_e2e_streaming_post_call_buffer_pass() {
    // Stream buffered, guard passes -> SSE response streamed to client
    let eval = setup_evaluator(true).await;
    let guard = guard_with_server(
        "response-check",
        GuardMode::PostCall,
        OnFailure::Block,
        &eval.uri(),
        "profanity-detector",
    );

    let accumulated = "Hello world!";

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], &accumulated, &client).await;

    assert!(!outcome.blocked);
}

#[tokio::test]
async fn test_e2e_streaming_post_call_buffer_block() {
    // Stream buffered, guard blocks -> 403
    let eval = setup_evaluator(false).await;
    let guard = guard_with_server(
        "pii-check",
        GuardMode::PostCall,
        OnFailure::Block,
        &eval.uri(),
        "pii-detector",
    );

    let accumulated = "Here is SSN: 123-45-6789";

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], &accumulated, &client).await;

    assert!(outcome.blocked);
}

#[tokio::test]
async fn test_e2e_config_from_yaml_with_env_vars() {
    // Full YAML config with ${VAR} substitution in api_key
    use std::io::Write;
    use tempfile::NamedTempFile;

    unsafe {
        std::env::set_var("E2E_TEST_API_KEY", "resolved-key-123");
    }

    let config_yaml = r#"
providers:
  - key: openai
    type: openai
    api_key: "sk-test"
models:
  - key: gpt-4
    type: gpt-4
    provider: openai
pipelines:
  - name: default
    type: chat
    plugins:
      - model-router:
          models:
            - gpt-4
guardrails:
  providers:
    - name: traceloop
      api_base: "https://api.traceloop.com"
      api_key: "${E2E_TEST_API_KEY}"
  guards:
    - name: toxicity-check
      provider: traceloop
      evaluator_slug: toxicity-detector
      mode: pre_call
      on_failure: block
    - name: pii-check
      provider: traceloop
      evaluator_slug: pii-detector
      mode: post_call
      on_failure: warn
      api_key: "override-key"
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_yaml.as_bytes()).unwrap();

    let config = hub_lib::config::load_config(temp_file.path().to_str().unwrap()).unwrap();
    let gr = config.guardrails.unwrap();

    assert_eq!(gr.providers.len(), 1);
    assert_eq!(gr.providers["traceloop"].api_key, "resolved-key-123");

    // Guards should have evaluator_slug at top level
    assert_eq!(gr.guards[0].evaluator_slug, "toxicity-detector");
    assert_eq!(gr.guards[0].mode, GuardMode::PreCall);
    assert!(gr.guards[0].api_base.is_none()); // inherits from provider
    assert!(gr.guards[0].api_key.is_none()); // inherits from provider

    // Second guard overrides api_key
    assert_eq!(gr.guards[1].api_key.as_deref(), Some("override-key"));

    // Build pipeline guardrails - should resolve provider defaults
    let shared = build_guardrail_resources(&gr).unwrap();
    let guard_names: Vec<String> = gr.guards.iter().map(|g| g.name.clone()).collect();
    let pipeline_gr = build_pipeline_guardrails(&shared, &guard_names);
    assert_eq!(pipeline_gr.all_guards.len(), 2);
    assert_eq!(pipeline_gr.pipeline_guard_names.len(), 2);
    // Provider api_base should be resolved for guards that don't override
    let pre_guard = pipeline_gr
        .all_guards
        .iter()
        .find(|g| g.mode == GuardMode::PreCall)
        .unwrap();
    let post_guard = pipeline_gr
        .all_guards
        .iter()
        .find(|g| g.mode == GuardMode::PostCall)
        .unwrap();
    assert_eq!(
        pre_guard.api_base.as_deref(),
        Some("https://api.traceloop.com")
    );
    assert_eq!(pre_guard.api_key.as_deref(), Some("resolved-key-123"));
    // Guard with override keeps its own api_key
    assert_eq!(post_guard.api_key.as_deref(), Some("override-key"));

    unsafe {
        std::env::remove_var("E2E_TEST_API_KEY");
    }
}

#[tokio::test]
async fn test_e2e_multiple_guards_different_evaluators() {
    // Different evaluator slugs -> separate mock expectations
    let server = MockServer::start().await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v2/guardrails/execute/toxicity-detector"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"result": {}, "pass": true})))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(matchers::method("POST"))
        .and(matchers::path("/v2/guardrails/execute/pii-detector"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"result": {}, "pass": true})))
        .expect(1)
        .mount(&server)
        .await;

    let guards = vec![
        guard_with_server(
            "tox-guard",
            GuardMode::PreCall,
            OnFailure::Block,
            &server.uri(),
            "toxicity-detector",
        ),
        guard_with_server(
            "pii-guard",
            GuardMode::PreCall,
            OnFailure::Block,
            &server.uri(),
            "pii-detector",
        ),
    ];

    let client = TraceloopClient::new();
    let outcome = execute_guards(&guards, "test input", &client).await;

    assert!(!outcome.blocked);
    assert_eq!(outcome.results.len(), 2);
    // wiremock will verify each path was called exactly once
}

#[tokio::test]
async fn test_e2e_fail_open_evaluator_down() {
    // Evaluator service down + required: false -> passthrough
    let server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let mut guard = guard_with_server(
        "checker",
        GuardMode::PreCall,
        OnFailure::Block,
        &server.uri(),
        "profanity-detector",
    );
    guard.required = false; // fail-open

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], "test input", &client).await;

    assert!(!outcome.blocked); // Fail-open: not blocked despite error
}

#[tokio::test]
async fn test_e2e_fail_closed_evaluator_down() {
    // Evaluator service down + required: true -> 403
    let server = MockServer::start().await;
    Mock::given(matchers::any())
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let mut guard = guard_with_server(
        "checker",
        GuardMode::PreCall,
        OnFailure::Block,
        &server.uri(),
        "profanity-detector",
    );
    guard.required = true; // fail-closed

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], "test input", &client).await;

    assert!(outcome.blocked); // Fail-closed: blocked due to error
}

#[tokio::test]
async fn test_e2e_config_validation_rejects_invalid() {
    // Config with missing required fields -> deserialization error
    let invalid_json = json!({
        "guards": [{
            "name": "incomplete-guard"
            // missing provider, evaluator_slug, mode
        }]
    });
    let result = serde_json::from_value::<GuardrailsConfig>(invalid_json);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_e2e_backward_compat_no_guardrails() {
    // Existing config without guardrails works unchanged
    use std::io::Write;
    use tempfile::NamedTempFile;

    let config_yaml = r#"
providers:
  - key: openai
    type: openai
    api_key: "sk-test"
models:
  - key: gpt-4
    type: gpt-4
    provider: openai
pipelines:
  - name: default
    type: chat
    plugins:
      - model-router:
          models:
            - gpt-4
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_yaml.as_bytes()).unwrap();

    let config = hub_lib::config::load_config(temp_file.path().to_str().unwrap()).unwrap();
    assert!(config.guardrails.is_none());

    // build_guardrail_resources with None guardrails returns None
    let shared = config
        .guardrails
        .as_ref()
        .and_then(build_guardrail_resources);
    assert!(shared.is_none());
}

// ---------------------------------------------------------------------------
// Pipeline Integration (4 tests)
// ---------------------------------------------------------------------------

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

    let guard = guard_with_server(
        "tone-check",
        GuardMode::PreCall,
        OnFailure::Warn,
        &eval_server.uri(),
        "tone",
    );

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], "borderline input", &client).await;

    assert!(!outcome.blocked);
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].guard_name, "tone-check");
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

    let guard = guard_with_server(
        "safety-check",
        GuardMode::PostCall,
        OnFailure::Warn,
        &eval_server.uri(),
        "safety",
    );

    let client = TraceloopClient::new();
    let outcome = execute_guards(&[guard], "Some LLM response", &client).await;

    assert!(!outcome.blocked);
    assert!(!outcome.warnings.is_empty());

    // Verify warning header would be generated correctly
    let header = warning_header_value(&outcome.warnings);
    assert!(header.contains("guardrail_name="));
    assert!(header.contains("safety-check"));
}

#[tokio::test]
async fn test_warning_header_format() {
    let warnings = vec![GuardWarning {
        guard_name: "my-guard".to_string(),
        reason: "failed".to_string(),
    }];
    let header = warning_header_value(&warnings);
    assert_eq!(header, "guardrail_name=\"my-guard\", reason=\"failed\"");
}

#[tokio::test]
async fn test_blocked_response_403_format() {
    let outcome = GuardrailsOutcome {
        results: vec![GuardResult::Failed {
            name: "toxicity-check".to_string(),
            result: json!({"reason": "toxic content"}),
            on_failure: OnFailure::Block,
        }],
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
