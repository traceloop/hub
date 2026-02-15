use std::collections::HashMap;
use hub_lib::guardrails::api_control::{resolve_guards_by_name, split_guards_by_mode};
use hub_lib::guardrails::executor::execute_guards;
use hub_lib::guardrails::providers::traceloop::TraceloopClient;
use hub_lib::guardrails::types::*;
use hub_lib::pipelines::pipeline::{
    blocked_response, build_guardrail_resources, build_pipeline_guardrails, resolve_guard_defaults,
    warning_header_value,
};

use axum::body::to_bytes;
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

    let guard = Guard {
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

    let guard = Guard {
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
    assert_eq!(outcome.warnings[0].guard_name, "tone-check");
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

    let guard = Guard {
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

    let guard = Guard {
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
    let blocking_guard = Some("toxicity-check".to_string());
    let response = blocked_response(&blocking_guard);
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
    // Empty guardrails config -> build_guardrail_resources returns None
    let config = GuardrailsConfig {
        providers: Default::default(),
        guards: vec![],
    };
    let result = build_guardrail_resources(&config);
    assert!(result.is_none());

    // Config with no guards -> passthrough
    let config_with_providers = GuardrailsConfig {
        providers: HashMap::from([("traceloop".to_string(), ProviderConfig {
            name: "traceloop".to_string(),
            api_base: "http://localhost".to_string(),
            api_key: "key".to_string(),
        })]),
        guards: vec![],
    };
    let result = build_guardrail_resources(&config_with_providers);
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Pipeline-specific guard association tests
// ---------------------------------------------------------------------------

fn test_guardrails_config() -> GuardrailsConfig {
    GuardrailsConfig {
        providers: HashMap::from([("traceloop".to_string(), ProviderConfig {
            name: "traceloop".to_string(),
            api_base: "https://api.traceloop.com".to_string(),
            api_key: "test-key".to_string(),
        })]),
        guards: vec![
            Guard {
                name: "pii-check".to_string(),
                provider: "traceloop".to_string(),
                evaluator_slug: "pii".to_string(),
                params: Default::default(),
                mode: GuardMode::PreCall,
                on_failure: OnFailure::Block,
                required: true,
                api_base: None,
                api_key: None,
            },
            Guard {
                name: "toxicity-filter".to_string(),
                provider: "traceloop".to_string(),
                evaluator_slug: "toxicity".to_string(),
                params: Default::default(),
                mode: GuardMode::PostCall,
                on_failure: OnFailure::Warn,
                required: true,
                api_base: None,
                api_key: None,
            },
            Guard {
                name: "injection-check".to_string(),
                provider: "traceloop".to_string(),
                evaluator_slug: "injection".to_string(),
                params: Default::default(),
                mode: GuardMode::PreCall,
                on_failure: OnFailure::Block,
                required: true,
                api_base: None,
                api_key: None,
            },
            Guard {
                name: "secrets-check".to_string(),
                provider: "traceloop".to_string(),
                evaluator_slug: "secrets".to_string(),
                params: Default::default(),
                mode: GuardMode::PostCall,
                on_failure: OnFailure::Block,
                required: true,
                api_base: None,
                api_key: None,
            },
        ],
    }
}

#[test]
fn test_build_pipeline_guardrails_with_specific_guards() {
    let config = test_guardrails_config();
    let shared = build_guardrail_resources(&config).unwrap();
    let pipeline_guards = vec!["pii-check".to_string(), "toxicity-filter".to_string()];
    let gr = build_pipeline_guardrails(&shared, &pipeline_guards);

    // all_guards should contain ALL guards from config, resolved with provider defaults
    assert_eq!(gr.all_guards.len(), 4);
    // pipeline_guard_names should only contain the ones specified
    assert_eq!(gr.pipeline_guard_names, vec!["pii-check", "toxicity-filter"]);
}

#[test]
fn test_build_pipeline_guardrails_empty_pipeline_guards() {
    let config = test_guardrails_config();
    let shared = build_guardrail_resources(&config).unwrap();
    // Pipeline with no guards specified - shared resources still exist
    // (header guards can still be used at request time)
    let gr = build_pipeline_guardrails(&shared);

    assert_eq!(gr.all_guards.len(), 4);
    assert!(gr.pipeline_guard_names.is_empty());
}

#[test]
fn test_build_pipeline_guardrails_resolves_provider_defaults() {
    let config = test_guardrails_config();
    let shared = build_guardrail_resources(&config).unwrap();
    let gr = build_pipeline_guardrails(&shared, &["pii-check".to_string()]);

    // Guards should have provider api_base/api_key resolved
    for guard in gr.all_guards.iter() {
        assert_eq!(guard.api_base.as_deref(), Some("https://api.traceloop.com"));
        assert_eq!(guard.api_key.as_deref(), Some("test-key"));
    }
}

#[test]
fn test_resolve_guard_defaults_preserves_guard_overrides() {
    let config = GuardrailsConfig {
        providers: HashMap::from([("traceloop".to_string(), ProviderConfig {
            name: "traceloop".to_string(),
            api_base: "https://default.api.com".to_string(),
            api_key: "default-key".to_string(),
        })]),
        guards: vec![Guard {
            name: "custom-guard".to_string(),
            provider: "traceloop".to_string(),
            evaluator_slug: "custom".to_string(),
            params: Default::default(),
            mode: GuardMode::PreCall,
            on_failure: OnFailure::Block,
            required: true,
            api_base: Some("https://custom.api.com".to_string()),
            api_key: Some("custom-key".to_string()),
        }],
    };

    let resolved = resolve_guard_defaults(&config);
    assert_eq!(resolved[0].api_base.as_deref(), Some("https://custom.api.com"));
    assert_eq!(resolved[0].api_key.as_deref(), Some("custom-key"));
}

#[test]
fn test_pipeline_guards_resolved_at_request_time() {
    // Simulates what happens at request time: merge pipeline + header guards
    let config = test_guardrails_config();
    let all_guards = resolve_guard_defaults(&config);

    // Pipeline declares only pii-check
    let pipeline_names = vec!["pii-check"];
    // Header adds injection-check
    let header_names = vec!["injection-check"];

    let resolved = resolve_guards_by_name(
        &all_guards,
        &pipeline_names,
        &header_names,
    );
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].name, "pii-check");
    assert_eq!(resolved[1].name, "injection-check");
}

#[test]
fn test_pipeline_guards_plus_header_guards_split_by_mode() {
    let config = test_guardrails_config();
    let all_guards = resolve_guard_defaults(&config);

    // Pipeline declares pii-check (pre_call) and toxicity-filter (post_call)
    let pipeline_names = vec!["pii-check", "toxicity-filter"];
    // Header adds injection-check (pre_call) and secrets-check (post_call)
    let header_names = vec!["injection-check", "secrets-check"];

    let resolved = resolve_guards_by_name(&all_guards, &pipeline_names, &header_names);
    assert_eq!(resolved.len(), 4);

    let (pre_call, post_call) = split_guards_by_mode(&resolved);
    assert_eq!(pre_call.len(), 2);
    assert_eq!(post_call.len(), 2);
    assert!(pre_call.iter().any(|g| g.name == "pii-check"));
    assert!(pre_call.iter().any(|g| g.name == "injection-check"));
    assert!(post_call.iter().any(|g| g.name == "toxicity-filter"));
    assert!(post_call.iter().any(|g| g.name == "secrets-check"));
}

#[test]
fn test_header_guard_not_in_config_is_ignored() {
    let config = test_guardrails_config();
    let all_guards = resolve_guard_defaults(&config);

    let pipeline_names = vec!["pii-check"];
    let header_names = vec!["nonexistent-guard"];

    let resolved = resolve_guards_by_name(&all_guards, &pipeline_names, &header_names);
    // Only pii-check should be resolved; nonexistent guard is silently ignored
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "pii-check");
}

#[test]
fn test_duplicate_guard_in_header_and_pipeline_deduped() {
    let config = test_guardrails_config();
    let all_guards = resolve_guard_defaults(&config);

    let pipeline_names = vec!["pii-check", "toxicity-filter"];
    // Header specifies same guard as pipeline
    let header_names = vec!["pii-check"];

    let resolved = resolve_guards_by_name(&all_guards, &pipeline_names, &header_names);
    assert_eq!(resolved.len(), 2); // pii-check only appears once
}

#[test]
fn test_no_pipeline_guards_header_only() {
    let config = test_guardrails_config();
    let all_guards = resolve_guard_defaults(&config);

    // Pipeline has no guards
    let pipeline_names: Vec<&str> = vec![];
    // Header adds guards
    let header_names = vec!["injection-check", "secrets-check"];

    let resolved = resolve_guards_by_name(&all_guards, &pipeline_names, &header_names);
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].name, "injection-check");
    assert_eq!(resolved[1].name, "secrets-check");
}

#[test]
fn test_no_pipeline_guards_no_header_no_guards_executed() {
    let config = test_guardrails_config();
    let all_guards = resolve_guard_defaults(&config);

    let resolved = resolve_guards_by_name(&all_guards, &[], &[]);
    assert!(resolved.is_empty());

    let (pre_call, post_call) = split_guards_by_mode(&resolved);
    assert!(pre_call.is_empty());
    assert!(post_call.is_empty());
}

#[test]
fn test_pipeline_guards_field_in_yaml_config() {
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
    guards:
      - pii-check
      - injection-check
    plugins:
      - model-router:
          models:
            - gpt-4
  - name: embeddings
    type: embeddings
    plugins:
      - model-router:
          models:
            - gpt-4
guardrails:
  providers:
    - name: traceloop
      api_base: "https://api.traceloop.com"
      api_key: "test-key"
  guards:
    - name: pii-check
      provider: traceloop
      evaluator_slug: pii
      mode: pre_call
      on_failure: block
    - name: injection-check
      provider: traceloop
      evaluator_slug: injection
      mode: pre_call
      on_failure: block
"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_yaml.as_bytes()).unwrap();

    let config = hub_lib::config::load_config(temp_file.path().to_str().unwrap()).unwrap();

    // Default pipeline should have guards
    assert_eq!(config.pipelines[0].guards, vec!["pii-check", "injection-check"]);
    // Embeddings pipeline should have no guards
    assert!(config.pipelines[1].guards.is_empty());
}

#[test]
fn test_pipeline_guards_field_absent_defaults_to_empty() {
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
    assert!(config.pipelines[0].guards.is_empty());
}
