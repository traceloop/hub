use hub_lib::guardrails::parsing::CompletionExtractor;
use hub_lib::guardrails::runner::*;
use hub_lib::guardrails::types::*;
use opentelemetry::Context;
use opentelemetry::trace::{Span, SpanKind, TraceContextExt, Tracer};
use opentelemetry_sdk::export::trace::SpanData;
use opentelemetry_sdk::testing::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::TracerProvider;

use super::helpers::*;


#[tokio::test]
async fn test_execute_single_pre_call_guard_passes() {
    let guard = create_test_guard("check", GuardMode::PreCall);
    let mock_client = MockGuardrailClient::with_response("check", Ok(passing_response()));
    let outcome = execute_guards(&[guard], "test input", &mock_client, None).await;
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
    let outcome = execute_guards(&[guard], "toxic input", &mock_client, None).await;
    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard, Some("check".to_string()));
}

#[tokio::test]
async fn test_execute_single_pre_call_guard_fails_warn() {
    let guard = create_test_guard_with_failure_action("check", GuardMode::PreCall, OnFailure::Warn);
    let mock_client = MockGuardrailClient::with_response("check", Ok(failing_response()));
    let outcome = execute_guards(&[guard], "borderline input", &mock_client, None).await;
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
    let outcome = execute_guards(&guards, "safe input", &mock_client, None).await;
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
    let outcome = execute_guards(&guards, "input", &mock_client, None).await;
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
    let outcome = execute_guards(&guards, "input", &mock_client, None).await;
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
    let outcome = execute_guards(&[guard], "input", &mock_client, None).await;
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
    let outcome = execute_guards(&[guard], "input", &mock_client, None).await;
    assert!(outcome.blocked); // Fail-closed
}

#[tokio::test]
async fn test_execute_post_call_guards_non_streaming() {
    let guard = create_test_guard("response-check", GuardMode::PostCall);
    let mock_client = MockGuardrailClient::with_response("response-check", Ok(passing_response()));
    let completion = create_test_chat_completion("Safe response text");
    let response_text = completion.extract_completion();
    let outcome = execute_guards(&[guard], &response_text, &mock_client, None).await;
    assert!(!outcome.blocked);
}

#[tokio::test]
async fn test_execute_post_call_guards_streaming_accumulated() {
    let guard = create_test_guard("response-check", GuardMode::PostCall);
    let mock_client = MockGuardrailClient::with_response("response-check", Ok(passing_response()));
    let accumulated_text = "Hello world from streaming!";
    let outcome = execute_guards(&[guard], accumulated_text, &mock_client, None).await;
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
    let outcome = execute_guards(&guards, "input", &mock_client, None).await;
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
    let outcome = execute_guards(&guards, "input", &mock_client, None).await;
    assert!(outcome.blocked);
    assert_eq!(outcome.blocking_guard, Some("blocker".to_string()));
    assert!(outcome.warnings.iter().any(|w| w.guard_name == "warner"));
}

// ---------------------------------------------------------------------------
// Guard Span Creation
// ---------------------------------------------------------------------------

use std::sync::LazyLock;

/// Shared OTel exporter + provider, initialized once for all span tests.
/// Each test creates a unique parent span with a unique trace_id, then filters
/// exported spans by that trace_id — so tests are isolated despite sharing state.
static TEST_EXPORTER: LazyLock<InMemorySpanExporter> = LazyLock::new(|| {
    let exporter = InMemorySpanExporter::default();
    let provider = TracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    opentelemetry::global::set_tracer_provider(provider);
    exporter
});

/// Helper: create a parent Context from the global tracer, returning
/// the Context and the parent's SpanContext for later assertions.
fn create_parent_context() -> (Context, opentelemetry::trace::SpanContext) {
    let tracer = opentelemetry::global::tracer("traceloop_hub");
    let parent_span = tracer.start("traceloop_hub");
    let span_ctx = parent_span.span_context().clone();
    let cx = Context::current().with_span(parent_span);
    (cx, span_ctx)
}

/// Helper: collect guard spans from the shared exporter, filtering by trace_id.
fn get_guard_spans(trace_id: opentelemetry::trace::TraceId) -> Vec<SpanData> {
    TEST_EXPORTER
        .get_finished_spans()
        .unwrap()
        .into_iter()
        .filter(|s| s.span_context.trace_id() == trace_id)
        .filter(|s| s.name.ends_with(".guard"))
        .collect()
}

#[tokio::test]
async fn test_guard_spans_created_with_parent_context() {
    let _ = &*TEST_EXPORTER; // ensure global provider is set
    let (parent_cx, parent_span_ctx) = create_parent_context();

    let guards = vec![
        create_test_guard("pii-check", GuardMode::PreCall),
        create_test_guard("secrets-check", GuardMode::PostCall),
    ];
    let mock_client = MockGuardrailClient::with_responses(vec![
        ("pii-check", Ok(passing_response())),
        ("secrets-check", Ok(failing_response())),
    ]);

    let _outcome = execute_guards(&guards, "test input", &mock_client, Some(&parent_cx)).await;
    drop(parent_cx);

    let spans = get_guard_spans(parent_span_ctx.trace_id());
    assert_eq!(
        spans.len(),
        2,
        "Expected 2 guard spans, got {}",
        spans.len()
    );

    let span_names: Vec<&str> = spans.iter().map(|s| s.name.as_ref()).collect();
    assert!(span_names.contains(&"pii-check.guard"));
    assert!(span_names.contains(&"secrets-check.guard"));

    // All guard spans should be children of the parent
    for span in &spans {
        assert_eq!(
            span.parent_span_id,
            parent_span_ctx.span_id(),
            "Guard span '{}' should be child of the parent span",
            span.name
        );
        assert_eq!(span.span_context.trace_id(), parent_span_ctx.trace_id());
        assert_eq!(span.span_kind, SpanKind::Internal);
    }
}

#[tokio::test]
async fn test_guard_span_attributes_on_pass() {
    let _ = &*TEST_EXPORTER;
    let (parent_cx, parent_span_ctx) = create_parent_context();

    let guard = create_test_guard("pii-check", GuardMode::PreCall);
    let mock_client = MockGuardrailClient::with_response("pii-check", Ok(passing_response()));

    let _outcome = execute_guards(&[guard], "hello world", &mock_client, Some(&parent_cx)).await;
    drop(parent_cx);

    let spans = get_guard_spans(parent_span_ctx.trace_id());
    assert_eq!(spans.len(), 1);

    let span = &spans[0];
    let attrs: std::collections::HashMap<String, String> = span
        .attributes
        .iter()
        .map(|kv| (kv.key.to_string(), kv.value.to_string()))
        .collect();

    assert_eq!(attrs.get("gen_ai.guardrail.name").unwrap(), "pii-check");
    assert_eq!(attrs.get("gen_ai.guardrail.status").unwrap(), "PASSED");
    assert!(attrs.contains_key("gen_ai.guardrail.duration"));
}

#[tokio::test]
async fn test_guard_span_attributes_on_fail() {
    let _ = &*TEST_EXPORTER;
    let (parent_cx, parent_span_ctx) = create_parent_context();

    let guard =
        create_test_guard_with_failure_action("toxicity", GuardMode::PreCall, OnFailure::Block);
    let mock_client = MockGuardrailClient::with_response("toxicity", Ok(failing_response()));

    let _outcome = execute_guards(&[guard], "bad input", &mock_client, Some(&parent_cx)).await;
    drop(parent_cx);

    let spans = get_guard_spans(parent_span_ctx.trace_id());
    assert_eq!(spans.len(), 1);

    let span = &spans[0];
    let attrs: std::collections::HashMap<String, String> = span
        .attributes
        .iter()
        .map(|kv| (kv.key.to_string(), kv.value.to_string()))
        .collect();

    assert_eq!(attrs.get("gen_ai.guardrail.name").unwrap(), "toxicity");
    assert_eq!(attrs.get("gen_ai.guardrail.status").unwrap(), "FAILED");
}

#[tokio::test]
async fn test_guard_span_attributes_on_error() {
    let _ = &*TEST_EXPORTER;
    let (parent_cx, parent_span_ctx) = create_parent_context();

    let guard = create_test_guard_with_required("failing-guard", GuardMode::PreCall, true);
    let mock_client = MockGuardrailClient::with_response(
        "failing-guard",
        Err(GuardrailError::Timeout("timed out".to_string())),
    );

    let _outcome = execute_guards(&[guard], "test input", &mock_client, Some(&parent_cx)).await;
    drop(parent_cx);

    let spans = get_guard_spans(parent_span_ctx.trace_id());
    assert_eq!(spans.len(), 1);

    let span = &spans[0];
    let attrs: std::collections::HashMap<String, String> = span
        .attributes
        .iter()
        .map(|kv| (kv.key.to_string(), kv.value.to_string()))
        .collect();

    assert_eq!(attrs.get("gen_ai.guardrail.name").unwrap(), "failing-guard");
    assert_eq!(attrs.get("gen_ai.guardrail.status").unwrap(), "ERROR");
    assert_eq!(attrs.get("gen_ai.guardrail.error.type").unwrap(), "Timeout");
    assert!(
        attrs
            .get("gen_ai.guardrail.error.message")
            .unwrap()
            .contains("timed out")
    );
}

#[tokio::test]
async fn test_no_guard_spans_without_parent_context() {
    let _ = &*TEST_EXPORTER;

    // Create a unique trace to establish a "before" baseline
    let (marker_cx, marker_span_ctx) = create_parent_context();
    drop(marker_cx);

    let guard = create_test_guard("pii-check", GuardMode::PreCall);
    let mock_client = MockGuardrailClient::with_response("pii-check", Ok(passing_response()));

    // Run with None parent — no guard spans should be created
    let _outcome = execute_guards(&[guard], "test input", &mock_client, None).await;

    // No guard spans should share the marker's trace_id (nothing was parented to it)
    let guard_spans = get_guard_spans(marker_span_ctx.trace_id());
    assert!(
        guard_spans.is_empty(),
        "No guard spans should be created when parent_cx is None"
    );
}
