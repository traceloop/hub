use hub_lib::guardrails::types::*;
use hub_lib::types::GatewayConfig;
use std::io::Write;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// Phase 1: Core Types & Configuration (9 tests + 4 provider tests)
// ---------------------------------------------------------------------------

#[test]
fn test_guard_mode_deserialize_pre_call() {
    let mode: GuardMode = serde_json::from_str("\"pre_call\"").unwrap();
    assert_eq!(mode, GuardMode::PreCall);
}

#[test]
fn test_guard_mode_deserialize_post_call() {
    let mode: GuardMode = serde_json::from_str("\"post_call\"").unwrap();
    assert_eq!(mode, GuardMode::PostCall);
}

#[test]
fn test_on_failure_defaults_to_warn() {
    let json = serde_json::json!({
        "name": "test-guard",
        "provider": "traceloop",
        "evaluator_slug": "toxicity",
        "mode": "pre_call"
    });
    let guard: Guard = serde_json::from_value(json).unwrap();
    assert_eq!(guard.on_failure, OnFailure::Warn);
}

#[test]
fn test_required_defaults_to_true() {
    let json = serde_json::json!({
        "name": "test-guard",
        "provider": "traceloop",
        "evaluator_slug": "toxicity",
        "mode": "pre_call"
    });
    let guard: Guard = serde_json::from_value(json).unwrap();
    assert!(guard.required);
}

#[test]
fn test_guard_config_full_deserialization() {
    let json = serde_json::json!({
        "name": "toxicity-check",
        "provider": "traceloop",
        "evaluator_slug": "toxicity",
        "params": {
            "threshold": 0.5
        },
        "mode": "pre_call",
        "on_failure": "block",
        "required": false,
        "api_base": "https://api.traceloop.com",
        "api_key": "tl-key-123"
    });
    let guard: Guard = serde_json::from_value(json).unwrap();
    assert_eq!(guard.name, "toxicity-check");
    assert_eq!(guard.provider, "traceloop");
    assert_eq!(guard.evaluator_slug, "toxicity");
    assert_eq!(guard.params.get("threshold").unwrap(), 0.5);
    assert_eq!(guard.mode, GuardMode::PreCall);
    assert_eq!(guard.on_failure, OnFailure::Block);
    assert!(!guard.required);
    assert_eq!(guard.api_base.unwrap(), "https://api.traceloop.com");
    assert_eq!(guard.api_key.unwrap(), "tl-key-123");
}

#[test]
fn test_guardrails_config_yaml_deserialization() {
    let yaml = r#"
guards:
  - name: toxicity-check
    provider: traceloop
    evaluator_slug: toxicity
    mode: pre_call
    on_failure: block
    required: true
    api_base: "https://api.traceloop.com"
    api_key: "test-key"
  - name: relevance-check
    provider: traceloop
    evaluator_slug: relevance
    mode: post_call
    on_failure: warn
    api_base: "https://api.traceloop.com"
    api_key: "test-key"
"#;
    let config: GuardrailsConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.guards.len(), 2);
    assert_eq!(config.guards[0].name, "toxicity-check");
    assert_eq!(config.guards[0].evaluator_slug, "toxicity");
    assert_eq!(config.guards[0].mode, GuardMode::PreCall);
    assert_eq!(config.guards[1].name, "relevance-check");
    assert_eq!(config.guards[1].evaluator_slug, "relevance");
    assert_eq!(config.guards[1].mode, GuardMode::PostCall);
    assert_eq!(config.guards[1].on_failure, OnFailure::Warn);
}

#[test]
fn test_gateway_config_with_guardrails() {
    use super::helpers::create_test_guard;
    let config = GatewayConfig {
        general: None,
        providers: vec![],
        models: vec![],
        pipelines: vec![],
        guardrails: Some(GuardrailsConfig {
            providers: vec![],
            guards: vec![create_test_guard("test", GuardMode::PreCall)],
        }),
    };
    assert!(config.guardrails.is_some());
    assert_eq!(config.guardrails.unwrap().guards.len(), 1);
}

#[test]
fn test_gateway_config_without_guardrails_backward_compat() {
    let json = serde_json::json!({
        "providers": [],
        "models": [],
        "pipelines": []
    });
    let config: GatewayConfig = serde_json::from_value(json).unwrap();
    assert!(config.guardrails.is_none());
}

#[test]
fn test_guard_config_env_var_in_api_key() {
    unsafe {
        std::env::set_var("TEST_GUARD_API_KEY_UNIQUE", "tl-secret-key");
    }
    let config_content = r#"
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
  guards:
    - name: toxicity-check
      provider: traceloop
      evaluator_slug: toxicity
      mode: pre_call
      api_base: "https://api.traceloop.com"
      api_key: "${TEST_GUARD_API_KEY_UNIQUE}"
"#;
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(config_content.as_bytes()).unwrap();
    let config = hub_lib::config::load_config(temp_file.path().to_str().unwrap()).unwrap();
    let guards = config.guardrails.unwrap().guards;
    assert_eq!(guards[0].api_key.as_deref(), Some("tl-secret-key"));
    unsafe {
        std::env::remove_var("TEST_GUARD_API_KEY_UNIQUE");
    }
}

// ---------------------------------------------------------------------------
// Provider config tests
// ---------------------------------------------------------------------------

#[test]
fn test_provider_config_deserialization() {
    let json = serde_json::json!({
        "name": "traceloop",
        "api_base": "https://api.traceloop.com",
        "api_key": "tl-key-123"
    });
    let provider: ProviderConfig = serde_json::from_value(json).unwrap();
    assert_eq!(provider.name, "traceloop");
    assert_eq!(provider.api_base, "https://api.traceloop.com");
    assert_eq!(provider.api_key, "tl-key-123");
}

#[test]
fn test_guardrails_config_with_providers_yaml() {
    let yaml = r#"
providers:
  - name: traceloop
    api_base: "https://api.traceloop.com"
    api_key: "tl-key-123"
guards:
  - name: toxicity-check
    provider: traceloop
    evaluator_slug: toxicity
    mode: pre_call
    on_failure: block
  - name: pii-check
    provider: traceloop
    evaluator_slug: pii-detection
    mode: pre_call
    on_failure: block
    api_base: "https://custom.traceloop.com"
    api_key: "custom-key"
"#;
    let config: GuardrailsConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.providers.len(), 1);
    assert_eq!(config.providers[0].name, "traceloop");
    assert_eq!(config.providers[0].api_base, "https://api.traceloop.com");
    assert_eq!(config.guards.len(), 2);
    // First guard has no api_base/api_key (inherits from provider)
    assert!(config.guards[0].api_base.is_none());
    assert!(config.guards[0].api_key.is_none());
    // Second guard overrides api_base/api_key
    assert_eq!(
        config.guards[1].api_base.as_deref(),
        Some("https://custom.traceloop.com")
    );
    assert_eq!(config.guards[1].api_key.as_deref(), Some("custom-key"));
}

#[test]
fn test_guard_without_api_base_deserializes() {
    let json = serde_json::json!({
        "name": "toxicity-check",
        "provider": "traceloop",
        "evaluator_slug": "toxicity",
        "mode": "pre_call"
    });
    let guard: Guard = serde_json::from_value(json).unwrap();
    assert!(guard.api_base.is_none());
    assert!(guard.api_key.is_none());
}

#[test]
fn test_guard_config_evaluator_slug_not_in_params() {
    let json = serde_json::json!({
        "name": "toxicity-check",
        "provider": "traceloop",
        "evaluator_slug": "toxicity",
        "params": {"threshold": 0.5},
        "mode": "pre_call"
    });
    let guard: Guard = serde_json::from_value(json).unwrap();
    assert_eq!(guard.evaluator_slug, "toxicity");
    assert!(!guard.params.contains_key("evaluator_slug"));
    assert_eq!(guard.params.get("threshold").unwrap(), 0.5);
}
