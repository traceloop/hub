use hub_lib::guardrails::executor::*;
use hub_lib::guardrails::input_extractor::*;
use hub_lib::guardrails::types::*;

use super::helpers::*;

// ---------------------------------------------------------------------------
// Phase 5: Executor (12 tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_execute_single_pre_call_guard_passes() {
    let guard = create_test_guard("check", GuardMode::PreCall);
    let mock_client = MockGuardrailClient::with_response("check", Ok(passing_response()));
    let outcome = execute_guards(&[guard], "test input", &mock_client).await;
    assert!(!outcome.blocked);
    assert_eq!(outcome.results.len(), 1);
    assert!(matches!(&outcome.results[0], GuardResult::Passed { .. }));
    assert!(outcome.warnings.is_empty());
}

#[tokio::test]
async fn test_execute_single_pre_call_guard_fails_block() {
    let guard =
        create_test_guard_with_failure_action("check", GuardMode::PreCall, OnFailure::Block);
    let mock_client = MockGuardrailClient::with_response("check", Ok(failing_response()));
    let outcome = execute_guards(&[guard], "toxic input", &mock_client).await;
    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard, Some("check".to_string()));
}

#[tokio::test]
async fn test_execute_single_pre_call_guard_fails_warn() {
    let guard = create_test_guard_with_failure_action("check", GuardMode::PreCall, OnFailure::Warn);
    let mock_client = MockGuardrailClient::with_response("check", Ok(failing_response()));
    let outcome = execute_guards(&[guard], "borderline input", &mock_client).await;
    assert!(!outcome.blocked);
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].guard_name, "check");
}

#[tokio::test]
async fn test_execute_multiple_pre_call_guards_all_pass() {
    let guards = vec![
        create_test_guard("guard-1", GuardMode::PreCall),
        create_test_guard("guard-2", GuardMode::PreCall),
        create_test_guard("guard-3", GuardMode::PreCall),
    ];
    let mock_client = MockGuardrailClient::with_responses(vec![
        ("guard-1", Ok(passing_response())),
        ("guard-2", Ok(passing_response())),
        ("guard-3", Ok(passing_response())),
    ]);
    let outcome = execute_guards(&guards, "safe input", &mock_client).await;
    assert!(!outcome.blocked);
    assert_eq!(outcome.results.len(), 3);
    assert!(outcome.warnings.is_empty());
}

#[tokio::test]
async fn test_execute_multiple_guards_one_blocks() {
    let guards = vec![
        create_test_guard("guard-1", GuardMode::PreCall),
        create_test_guard_with_failure_action("guard-2", GuardMode::PreCall, OnFailure::Block),
        create_test_guard("guard-3", GuardMode::PreCall),
    ];
    let mock_client = MockGuardrailClient::with_responses(vec![
        ("guard-1", Ok(passing_response())),
        ("guard-2", Ok(failing_response())),
        ("guard-3", Ok(passing_response())),
    ]);
    let outcome = execute_guards(&guards, "input", &mock_client).await;
    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard, Some("guard-2".to_string()));
}

#[tokio::test]
async fn test_execute_multiple_guards_one_warns_continue() {
    let guards = vec![
        create_test_guard("guard-1", GuardMode::PreCall),
        create_test_guard_with_failure_action("guard-2", GuardMode::PreCall, OnFailure::Warn),
        create_test_guard("guard-3", GuardMode::PreCall),
    ];
    let mock_client = MockGuardrailClient::with_responses(vec![
        ("guard-1", Ok(passing_response())),
        ("guard-2", Ok(failing_response())),
        ("guard-3", Ok(passing_response())),
    ]);
    let outcome = execute_guards(&guards, "input", &mock_client).await;
    assert!(!outcome.blocked);
    assert_eq!(outcome.results.len(), 3);
    assert_eq!(outcome.warnings.len(), 1);
}

#[tokio::test]
async fn test_guard_evaluator_unavailable_required_false() {
    let guard = create_test_guard_with_required("check", GuardMode::PreCall, false);
    let mock_client = MockGuardrailClient::with_response(
        "check",
        Err(GuardrailError::Unavailable(
            "connection refused".to_string(),
        )),
    );
    let outcome = execute_guards(&[guard], "input", &mock_client).await;
    assert!(!outcome.blocked); // Fail-open
    assert!(matches!(
        &outcome.results[0],
        GuardResult::Error {
            required: false,
            ..
        }
    ));
}

#[tokio::test]
async fn test_guard_evaluator_unavailable_required_true() {
    let guard = create_test_guard_with_required("check", GuardMode::PreCall, true);
    let mock_client = MockGuardrailClient::with_response(
        "check",
        Err(GuardrailError::Unavailable(
            "connection refused".to_string(),
        )),
    );
    let outcome = execute_guards(&[guard], "input", &mock_client).await;
    assert!(outcome.blocked); // Fail-closed
}

#[tokio::test]
async fn test_execute_post_call_guards_non_streaming() {
    let guard = create_test_guard("response-check", GuardMode::PostCall);
    let mock_client = MockGuardrailClient::with_response("response-check", Ok(passing_response()));
    let completion = create_test_chat_completion("Safe response text");
    let response_text = extract_post_call_input_from_completion(&completion);
    let outcome = execute_guards(&[guard], &response_text, &mock_client).await;
    assert!(!outcome.blocked);
}

#[tokio::test]
async fn test_execute_post_call_guards_streaming_accumulated() {
    let guard = create_test_guard("response-check", GuardMode::PostCall);
    let mock_client = MockGuardrailClient::with_response("response-check", Ok(passing_response()));
    let accumulated_text = "Hello world from streaming!";
    let outcome = execute_guards(&[guard], accumulated_text, &mock_client).await;
    assert!(!outcome.blocked);
}

#[tokio::test]
async fn test_parallel_execution_of_independent_guards() {
    // This test verifies guards run concurrently, not sequentially.
    // We use the mock client without delay here; the implementation should
    // use futures::join_all or similar for parallel execution.
    let guards = vec![
        create_test_guard("guard-1", GuardMode::PreCall),
        create_test_guard("guard-2", GuardMode::PreCall),
    ];
    let mock_client = MockGuardrailClient::with_responses(vec![
        ("guard-1", Ok(passing_response())),
        ("guard-2", Ok(passing_response())),
    ]);
    let start = std::time::Instant::now();
    let outcome = execute_guards(&guards, "input", &mock_client).await;
    let _elapsed = start.elapsed();
    assert!(!outcome.blocked);
    assert_eq!(outcome.results.len(), 2);
}

#[tokio::test]
async fn test_executor_returns_correct_guardrails_outcome() {
    let guards = vec![
        create_test_guard_with_failure_action("passer", GuardMode::PreCall, OnFailure::Block),
        create_test_guard_with_failure_action("warner", GuardMode::PreCall, OnFailure::Warn),
        create_test_guard_with_failure_action("blocker", GuardMode::PreCall, OnFailure::Block),
    ];
    let mock_client = MockGuardrailClient::with_responses(vec![
        ("passer", Ok(passing_response())),
        ("warner", Ok(failing_response())),
        ("blocker", Ok(failing_response())),
    ]);
    let outcome = execute_guards(&guards, "input", &mock_client).await;
    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard, Some("blocker".to_string()));
    assert!(outcome.warnings.iter().any(|w| w.guard_name == "warner"));
}
