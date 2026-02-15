use std::collections::HashMap;

use hub_lib::guardrails::setup::*;
use hub_lib::guardrails::types::*;

use super::helpers::*;

#[test]
fn test_parse_guardrails_header_single() {
    let names = parse_guardrails_header("pii-check");
    assert_eq!(names, vec!["pii-check"]);
}

#[test]
fn test_parse_guardrails_header_multiple() {
    let names = parse_guardrails_header("toxicity-check,relevance-check,pii-check");
    assert_eq!(
        names,
        vec!["toxicity-check", "relevance-check", "pii-check"]
    );
}

#[test]
fn test_pipeline_guardrails_always_included() {
    let pipeline_guards = vec![create_test_guard("pipeline-guard", GuardMode::PreCall)];
    let resolved = resolve_guards_by_name(&pipeline_guards, &["pipeline-guard"], &[]);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "pipeline-guard");
}

#[test]
fn test_header_guardrails_additive_to_pipeline() {
    let all_guards = vec![
        create_test_guard("pipeline-guard", GuardMode::PreCall),
        create_test_guard("header-guard", GuardMode::PreCall),
    ];
    let resolved = resolve_guards_by_name(&all_guards, &["pipeline-guard"], &["header-guard"]);
    assert_eq!(resolved.len(), 2);
}

#[test]
fn test_deduplication_by_name() {
    let all_guards = vec![create_test_guard("shared-guard", GuardMode::PreCall)];
    let resolved = resolve_guards_by_name(
        &all_guards,
        &["shared-guard"],
        &["shared-guard"], // duplicate
    );
    assert_eq!(resolved.len(), 1);
}

#[test]
fn test_unknown_guard_name_in_header_ignored() {
    let all_guards = vec![create_test_guard("known-guard", GuardMode::PreCall)];
    let resolved = resolve_guards_by_name(
        &all_guards,
        &["known-guard"],
        &["nonexistent-guard"], // unknown
    );
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "known-guard");
}

#[test]
fn test_empty_header_pipeline_guards_only() {
    let all_guards = vec![create_test_guard("pipeline-guard", GuardMode::PreCall)];
    let resolved = resolve_guards_by_name(&all_guards, &["pipeline-guard"], &[]);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "pipeline-guard");
}

#[test]
fn test_cannot_remove_pipeline_guardrails_via_api() {
    let all_guards = vec![
        create_test_guard("pipeline-guard", GuardMode::PreCall),
        create_test_guard("extra", GuardMode::PreCall),
    ];
    // Even if header/payload only mention "extra", pipeline guard still included
    let resolved = resolve_guards_by_name(&all_guards, &["pipeline-guard"], &["extra"]);
    assert!(resolved.iter().any(|g| g.name == "pipeline-guard"));
}

#[test]
fn test_guards_split_into_pre_and_post_call() {
    let guards = vec![
        create_test_guard("pre-1", GuardMode::PreCall),
        create_test_guard("post-1", GuardMode::PostCall),
        create_test_guard("pre-2", GuardMode::PreCall),
        create_test_guard("post-2", GuardMode::PostCall),
    ];
    let (pre_call, post_call) = split_guards_by_mode(&guards);
    assert_eq!(pre_call.len(), 2);
    assert_eq!(post_call.len(), 2);
    assert!(pre_call.iter().all(|g| g.mode == GuardMode::PreCall));
    assert!(post_call.iter().all(|g| g.mode == GuardMode::PostCall));
}

#[test]
fn test_complete_resolution_merged() {
    let all_guards = vec![
        create_test_guard("pipeline-pre", GuardMode::PreCall),
        create_test_guard("pipeline-post", GuardMode::PostCall),
        create_test_guard("header-pre", GuardMode::PreCall),
    ];
    let resolved = resolve_guards_by_name(
        &all_guards,
        &["pipeline-pre", "pipeline-post"],
        &["header-pre"],
    );
    assert_eq!(resolved.len(), 3);
    let (pre, post) = split_guards_by_mode(&resolved);
    assert_eq!(pre.len(), 2); // pipeline-pre + header-pre
    assert_eq!(post.len(), 1); // pipeline-post
}

// ---------------------------------------------------------------------------
// Pipeline Guard Building & Provider Defaults
// ---------------------------------------------------------------------------

fn test_guardrails_config() -> GuardrailsConfig {
    GuardrailsConfig {
        providers: HashMap::from([(
            "traceloop".to_string(),
            ProviderConfig {
                name: "traceloop".to_string(),
                api_base: "https://api.traceloop.com".to_string(),
                api_key: "test-key".to_string(),
            },
        )]),
        guards: vec![
            Guard {
                name: "pii-check".to_string(),
                provider: "traceloop".to_string(),
                evaluator_slug: "pii".to_string(),
                params: Default::default(),
                mode: GuardMode::PreCall,
                on_failure: OnFailure::Block,
                required: false,
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
                required: false,
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
                required: false,
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
                required: false,
                api_base: None,
                api_key: None,
            },
        ],
    }
}

#[test]
fn test_no_guardrails_passthrough() {
    // Empty guardrails config -> build_guardrail_resources returns None
    let config = GuardrailsConfig {
        providers: Default::default(),
        guards: vec![],
    };
    let result = build_guardrail_resources(&config);
    assert!(result.is_none());

    // Config with no guards -> passthrough
    let config_with_providers = GuardrailsConfig {
        providers: HashMap::from([(
            "traceloop".to_string(),
            ProviderConfig {
                name: "traceloop".to_string(),
                api_base: "http://localhost".to_string(),
                api_key: "key".to_string(),
            },
        )]),
        guards: vec![],
    };
    let result = build_guardrail_resources(&config_with_providers);
    assert!(result.is_none());
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
    assert_eq!(
        gr.pipeline_guard_names,
        vec!["pii-check", "toxicity-filter"]
    );
}

#[test]
fn test_build_pipeline_guardrails_empty_pipeline_guards() {
    let config = test_guardrails_config();
    let shared = build_guardrail_resources(&config).unwrap();
    // Pipeline with no guards specified - shared resources still exist
    // (header guards can still be used at request time)
    let empty: Vec<String> = vec![];
    let gr = build_pipeline_guardrails(&shared, &empty);

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
        providers: HashMap::from([(
            "traceloop".to_string(),
            ProviderConfig {
                name: "traceloop".to_string(),
                api_base: "https://default.api.com".to_string(),
                api_key: "default-key".to_string(),
            },
        )]),
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
    assert_eq!(
        resolved[0].api_base.as_deref(),
        Some("https://custom.api.com")
    );
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

    let resolved = resolve_guards_by_name(&all_guards, &pipeline_names, &header_names);
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
