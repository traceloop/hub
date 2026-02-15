use hub_lib::guardrails::setup::*;
use hub_lib::guardrails::types::GuardMode;

use super::helpers::*;

// ---------------------------------------------------------------------------
// Phase 7: API Control (15 tests)
// ---------------------------------------------------------------------------

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
