use std::collections::HashSet;

use axum::Json;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::future::join_all;
use opentelemetry::global::{BoxedSpan, ObjectSafeSpan};
use opentelemetry::trace::{SpanKind, Status as OtelStatus, Tracer};
use opentelemetry::{Context, KeyValue, global};
use serde_json::json;
use tracing::{debug, warn};

use crate::config::lib::get_trace_content_enabled;

use super::parsing::{CompletionExtractor, PromptExtractor};
use super::setup::{parse_guardrails_header, resolve_guards_by_name, split_guards_by_mode};
use super::span_attributes::*;
use super::types::{
    EvaluatorResponse, Guard, GuardResult, GuardWarning, GuardrailClient, GuardrailError,
    Guardrails, GuardrailsOutcome, OnFailure,
};

fn error_type_name(err: &GuardrailError) -> &'static str {
    match err {
        GuardrailError::Unavailable(_) => "Unavailable",
        GuardrailError::HttpError { .. } => "HttpError",
        GuardrailError::Timeout(_) => "Timeout",
        GuardrailError::ParseError(_) => "ParseError",
    }
}

fn record_guard_span(
    span: &mut BoxedSpan,
    guard: &Guard,
    result: &Result<EvaluatorResponse, GuardrailError>,
    elapsed: std::time::Duration,
    input: &str,
) {
    span.set_attribute(KeyValue::new(GEN_AI_GUARDRAIL_NAME, guard.name.clone()));
    span.set_attribute(KeyValue::new(
        GEN_AI_GUARDRAIL_DURATION,
        elapsed.as_millis() as i64,
    ));

    if get_trace_content_enabled() {
        span.set_attribute(KeyValue::new(GEN_AI_GUARDRAIL_INPUT, input.to_string()));
    }

    match result {
        Ok(resp) => {
            let status = if resp.pass {
                GUARDRAIL_PASSED
            } else {
                GUARDRAIL_FAILED
            };
            span.set_attribute(KeyValue::new(GEN_AI_GUARDRAIL_STATUS, status));
        }
        Err(err) => {
            span.set_attribute(KeyValue::new(GEN_AI_GUARDRAIL_STATUS, GUARDRAIL_ERROR));
            span.set_attribute(KeyValue::new(
                GEN_AI_GUARDRAIL_ERROR_TYPE,
                error_type_name(err),
            ));
            span.set_attribute(KeyValue::new(
                GEN_AI_GUARDRAIL_ERROR_MESSAGE,
                err.to_string(),
            ));
            span.set_status(OtelStatus::error(err.to_string()));
        }
    }
}

/// Execute a set of guardrails against the given input text.
/// Guards are run concurrently. Returns a GuardrailsOutcome with results, blocked status, and warnings.
/// When `parent_cx` is provided, creates a child OTel span per guard evaluation.
pub async fn execute_guards(
    guards: &[Guard],
    input: &str,
    client: &dyn GuardrailClient,
    parent_cx: Option<&Context>,
) -> GuardrailsOutcome {
    debug!(guard_count = guards.len(), "Executing guardrails");

    let parent_cx = parent_cx.cloned();

    let futures: Vec<_> = guards
        .iter()
        .map(|guard| {
            let parent_cx = parent_cx.clone();
            async move {
                // Create child span BEFORE evaluation so its start time is correct
                let mut span = parent_cx.as_ref().map(|cx| {
                    let tracer = global::tracer("traceloop_hub");
                    tracer
                        .span_builder(format!("{}.guard", guard.name))
                        .with_kind(SpanKind::Internal)
                        .start_with_context(&tracer, cx)
                });

                let start = std::time::Instant::now();
                let result = client.evaluate(guard, input).await;
                let elapsed = start.elapsed();

                if let Some(s) = &mut span {
                    record_guard_span(s, guard, &result, elapsed, input);
                }

                match &result {
                    Ok(resp) => debug!(
                        guard = %guard.name,
                        pass = resp.pass,
                        elapsed_ms = elapsed.as_millis(),
                        "Guard evaluation complete"
                    ),
                    Err(err) => warn!(
                        guard = %guard.name,
                        error = %err,
                        required = guard.required,
                        elapsed_ms = elapsed.as_millis(),
                        "Guard evaluation failed"
                    ),
                }
                (guard, result, span)
            }
        })
        .collect();

    let results_raw = join_all(futures).await;

    let mut results = Vec::new();
    let mut blocked = false;
    let mut blocking_guard = None;
    let mut warnings = Vec::new();
    let mut guard_spans: Vec<BoxedSpan> = Vec::new();

    for (guard, result, span) in results_raw {
        if let Some(s) = span {
            guard_spans.push(s);
        }
        let name = guard.name.clone();
        match result {
            Ok(response) => {
                if response.pass {
                    results.push(GuardResult::Passed { name });
                } else {
                    match guard.on_failure {
                        OnFailure::Block => {
                            blocked = true;
                            if blocking_guard.is_none() {
                                blocking_guard = Some(name.clone());
                            }
                        }
                        OnFailure::Warn => {
                            warnings.push(GuardWarning {
                                guard_name: name.clone(),
                                reason: "failed".to_string(),
                            });
                        }
                    }
                    results.push(GuardResult::Failed {
                        name,
                        result: response.result,
                        on_failure: guard.on_failure,
                    });
                }
            }
            Err(err) => {
                let is_required = guard.required;
                if is_required {
                    blocked = true;
                    if blocking_guard.is_none() {
                        blocking_guard = Some(name.clone());
                    }
                }
                results.push(GuardResult::Error {
                    name,
                    error: err.to_string(),
                    required: is_required,
                });
            }
        }
    }

    if blocked {
        warn!(blocking_guard = ?blocking_guard, "Request blocked by guardrail");
    }

    GuardrailsOutcome {
        results,
        blocked,
        blocking_guard,
        warnings,
    }
}

/// Result of a guard phase: Ok(warnings) on pass, Err(blocked_response) on block.
pub type GuardPhaseResult = Result<Vec<GuardWarning>, Box<Response>>;

pub struct GuardrailsRunner<'a> {
    pre_call: Vec<Guard>,
    post_call: Vec<Guard>,
    client: &'a dyn GuardrailClient,
    parent_cx: Option<Context>,
}

/// Convert a GuardrailsOutcome into a GuardPhaseResult.
/// If the outcome is blocked, produces a blocked response; otherwise, forwards warnings.
fn outcome_to_phase_result(outcome: GuardrailsOutcome) -> GuardPhaseResult {
    if outcome.blocked {
        Err(Box::new(blocked_response(&outcome)))
    } else {
        Ok(outcome.warnings)
    }
}

impl<'a> GuardrailsRunner<'a> {
    /// Create a runner by resolving guards from pipeline config + request headers.
    /// Returns None if no guards are active for this request.
    /// When `parent_cx` is provided, guardrail evaluations are traced as child spans.
    pub fn new(
        guardrails: Option<&'a Guardrails>,
        headers: &HeaderMap,
        parent_cx: Option<Context>,
    ) -> Option<Self> {
        let gr = guardrails?;
        let (pre_call, post_call) = resolve_request_guards(gr, headers);
        if pre_call.is_empty() && post_call.is_empty() {
            return None;
        }
        Some(Self {
            pre_call,
            post_call,
            client: gr.client.as_ref(),
            parent_cx,
        })
    }

    /// Run pre-call guards, extracting input from the request only if guards exist.
    pub async fn run_pre_call(&self, request: &impl PromptExtractor) -> GuardPhaseResult {
        if self.pre_call.is_empty() {
            return Ok(Vec::new());
        }
        let input = request.extract_prompt();
        let outcome =
            execute_guards(&self.pre_call, &input, self.client, self.parent_cx.as_ref()).await;
        outcome_to_phase_result(outcome)
    }

    /// Run post-call guards, extracting input from the response only if guards exist.
    pub async fn run_post_call(&self, response: &impl CompletionExtractor) -> GuardPhaseResult {
        if self.post_call.is_empty() {
            return Ok(Vec::new());
        }
        let completion = response.extract_completion();

        if completion.is_empty() {
            warn!("Skipping post-call guardrails: LLM response content is empty");
            return Ok(vec![GuardWarning {
                guard_name: "all post_call guards".to_string(),
                reason: "skipped due to empty response content".to_string(),
            }]);
        }

        let outcome = execute_guards(
            &self.post_call,
            &completion,
            self.client,
            self.parent_cx.as_ref(),
        )
        .await;
        outcome_to_phase_result(outcome)
    }

    /// Attach warning headers to a response if there are any warnings.
    /// Returns the response unchanged if there are no warnings.
    pub fn finalize_response(response: Response, warnings: &[GuardWarning]) -> Response {
        if warnings.is_empty() {
            return response;
        }
        let header_val = warning_header_value(warnings);
        let mut response = response;
        match header_val.parse() {
            Ok(parsed_header) => {
                response
                    .headers_mut()
                    .insert("x-traceloop-guardrail-warning", parsed_header);
            }
            Err(e) => {
                warn!(
                    error = %e,
                    header_value = %header_val,
                    "Failed to parse guardrail warning header, skipping header"
                );
            }
        }
        response
    }
}

/// Build a 403 blocked response with the guard name.
pub fn blocked_response(outcome: &GuardrailsOutcome) -> Response {
    let guard_name = outcome.blocking_guard.as_deref().unwrap_or("unknown");

    // Find the blocking guard result to get details
    let blocking_result = outcome.results.iter().find(|r| match r {
        GuardResult::Failed { name, .. } | GuardResult::Error { name, .. } => name == guard_name,
        _ => false,
    });

    let error_obj = match blocking_result {
        Some(GuardResult::Failed { result, .. }) => json!({
            "type": "guardrail_blocked",
            "guardrail": guard_name,
            "message": format!("Request blocked by guardrail '{guard_name}'"),
            "evaluation_result": result,
            "reason": "evaluation_failed",
        }),
        Some(GuardResult::Error { error, .. }) => json!({
            "type": "guardrail_blocked",
            "guardrail": guard_name,
            "message": format!("Request blocked by guardrail '{guard_name}'"),
            "error_details": error,
            "reason": "evaluator_error",
        }),
        _ => json!({
            "type": "guardrail_blocked",
            "guardrail": guard_name,
            "message": format!("Request blocked by guardrail '{guard_name}'"),
        }),
    };

    let body = json!({ "error": error_obj });
    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

pub fn warning_header_value(warnings: &[GuardWarning]) -> String {
    warnings
        .iter()
        .map(|w| {
            format!(
                "guardrail_name=\"{}\", reason=\"{}\"",
                w.guard_name, w.reason
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Resolve guards for this request by merging pipeline guards with header-specified guards.
fn resolve_request_guards(gr: &Guardrails, headers: &HeaderMap) -> (Vec<Guard>, Vec<Guard>) {
    let header_guard_names = headers
        .get("x-traceloop-guardrails")
        .and_then(|v| v.to_str().ok())
        .map(parse_guardrails_header)
        .unwrap_or_default();

    let pipeline_names: Vec<&str> = gr.pipeline_guard_names.iter().map(|s| s.as_str()).collect();
    let header_names: Vec<&str> = header_guard_names.iter().map(|s| s.as_str()).collect();
    let resolved = resolve_guards_by_name(&gr.all_guards, &pipeline_names, &header_names);

    // Log unknown guard names from headers
    if !header_guard_names.is_empty() {
        let resolved_names: HashSet<&str> = resolved.iter().map(|g| g.name.as_str()).collect();
        for name in &header_guard_names {
            if !resolved_names.contains(name.as_str()) {
                warn!(guard_name = %name, "Unknown guard name in X-Traceloop-Guardrails header, ignoring");
            }
        }
    }

    split_guards_by_mode(&resolved)
}
