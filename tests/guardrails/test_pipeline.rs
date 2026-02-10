// ---------------------------------------------------------------------------
// Phase 6: Pipeline Integration (7 tests)
//
// These tests verify that guardrails are properly wired into the pipeline
// request handling flow. They use wiremock for both the evaluator and LLM.
// ---------------------------------------------------------------------------

// TODO: These tests require pipeline integration implementation.
// They will be fully implemented when the pipeline hooks are added.
// For now, we define the test signatures to drive the implementation.

#[tokio::test]
async fn test_pre_call_guardrails_block_before_llm() {
    // Guard blocks the request -> 403 response, LLM receives 0 requests
    todo!("Implement pipeline integration test: pre_call block")
}

#[tokio::test]
async fn test_pre_call_guardrails_warn_and_continue() {
    // Guard warns but request proceeds to LLM -> 200 + warning header
    todo!("Implement pipeline integration test: pre_call warn")
}

#[tokio::test]
async fn test_post_call_guardrails_block_response() {
    // LLM responds, guard blocks output -> 403
    todo!("Implement pipeline integration test: post_call block")
}

#[tokio::test]
async fn test_post_call_guardrails_warn_and_add_header() {
    // LLM responds, guard warns -> 200 + X-Traceloop-Guardrail-Warning header
    todo!("Implement pipeline integration test: post_call warn")
}

#[tokio::test]
async fn test_warning_header_format() {
    // Warning header format: guardrail_name="...", reason="..."
    todo!("Implement pipeline integration test: warning header format")
}

#[tokio::test]
async fn test_blocked_response_403_format() {
    // Blocked response body: {"error": {"type": "guardrail_blocked", ...}}
    todo!("Implement pipeline integration test: 403 response format")
}

#[tokio::test]
async fn test_no_guardrails_passthrough() {
    // No guardrails configured -> normal passthrough behavior
    todo!("Implement pipeline integration test: passthrough")
}
