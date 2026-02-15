use std::collections::HashSet;

use axum::Json;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::future::join_all;
use serde_json::json;
use tracing::{debug, warn};

use super::parsing::{CompletionExtractor, PromptExtractor};
use super::setup::{parse_guardrails_header, resolve_guards_by_name, split_guards_by_mode};
use super::types::{
    Guard, GuardResult, GuardWarning, GuardrailClient, Guardrails, GuardrailsOutcome, OnFailure,
};

/// Execute a set of guardrails against the given input text.
/// Guards are run concurrently. Returns a GuardrailsOutcome with results, blocked status, and warnings.
pub async fn execute_guards(
    guards: &[Guard],
    input: &str,
    client: &dyn GuardrailClient,
) -> GuardrailsOutcome {
    debug!(guard_count = guards.len(), "Executing guardrails");

    let futures: Vec<_> = guards
        .iter()
        .map(|guard| async move {
            let start = std::time::Instant::now();
            let result = client.evaluate(guard, input).await;
            let elapsed = start.elapsed();
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
            (guard, result)
        })
        .collect();

    let results_raw = join_all(futures).await;

    let mut results = Vec::new();
    let mut blocked = false;
    let mut blocking_guard = None;
    let mut warnings = Vec::new();

    for (guard, result) in results_raw {
        match result {
            Ok(response) => {
                if response.pass {
                    results.push(GuardResult::Passed {
                        name: guard.name.clone(),
                    });
                } else {
                    results.push(GuardResult::Failed {
                        name: guard.name.clone(),
                        result: response.result,
                        on_failure: guard.on_failure.clone(),
                    });
                    match guard.on_failure {
                        OnFailure::Block => {
                            blocked = true;
                            if blocking_guard.is_none() {
                                blocking_guard = Some(guard.name.clone());
                            }
                        }
                        OnFailure::Warn => {
                            warnings.push(GuardWarning {
                                guard_name: guard.name.clone(),
                                reason: "failed".to_string(),
                            });
                        }
                    }
                }
            }
            Err(err) => {
                let is_required = guard.required;
                results.push(GuardResult::Error {
                    name: guard.name.clone(),
                    error: err.to_string(),
                    required: is_required,
                });
                if is_required {
                    blocked = true;
                    if blocking_guard.is_none() {
                        blocking_guard = Some(guard.name.clone());
                    }
                }
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

pub struct GuardPhaseResult {
    pub blocked_response: Option<Response>,
    pub warnings: Vec<GuardWarning>,
}

pub struct GuardrailsRunner<'a> {
    pre_call: Vec<Guard>,
    post_call: Vec<Guard>,
    client: &'a dyn GuardrailClient,
}

impl<'a> GuardrailsRunner<'a> {
    /// Create a runner by resolving guards from pipeline config + request headers.
    /// Returns None if no guards are active for this request.
    pub fn new(guardrails: Option<&'a Guardrails>, headers: &HeaderMap) -> Option<Self> {
        let gr = guardrails?;
        let (pre_call, post_call) = resolve_request_guards(gr, headers);
        if pre_call.is_empty() && post_call.is_empty() {
            return None;
        }
        Some(Self {
            pre_call,
            post_call,
            client: gr.client.as_ref(),
        })
    }

    /// Run pre-call guards, extracting input from the request only if guards exist.
    pub async fn run_pre_call(&self, request: &impl PromptExtractor) -> GuardPhaseResult {
        if self.pre_call.is_empty() {
            return GuardPhaseResult {
                blocked_response: None,
                warnings: Vec::new(),
            };
        }
        let input = request.extract_pompt();
        let outcome = execute_guards(&self.pre_call, &input, self.client).await;
        if outcome.blocked {
            return GuardPhaseResult {
                blocked_response: Some(blocked_response(&outcome)),
                warnings: Vec::new(),
            };
        }
        GuardPhaseResult {
            blocked_response: None,
            warnings: outcome.warnings,
        }
    }

    /// Run post-call guards, extracting input from the response only if guards exist.
    pub async fn run_post_call(&self, response: &impl CompletionExtractor) -> GuardPhaseResult {
        if self.post_call.is_empty() {
            return GuardPhaseResult {
                blocked_response: None,
                warnings: Vec::new(),
            };
        }
        let completion = response.extract_completion();

        if completion.is_empty() {
            warn!("Skipping post-call guardrails: LLM response content is empty");
            return GuardPhaseResult {
                blocked_response: None,
                warnings: vec![GuardWarning {
                    guard_name: "all post_call guards".to_string(),
                    reason: "skipped due to empty response content".to_string(),
                }],
            };
        }

        let outcome = execute_guards(&self.post_call, &completion, self.client).await;
        if outcome.blocked {
            return GuardPhaseResult {
                blocked_response: Some(blocked_response(&outcome)),
                warnings: Vec::new(),
            };
        }
        GuardPhaseResult {
            blocked_response: None,
            warnings: outcome.warnings,
        }
    }

    /// Attach warning headers to a response if there are any warnings.
    /// Returns the response unchanged if there are no warnings.
    pub fn finalize_response(response: Response, warnings: &[GuardWarning]) -> Response {
        if warnings.is_empty() {
            return response;
        }
        let header_val = warning_header_value(warnings);
        let mut response = response;
        response
            .headers_mut()
            .insert("X-Traceloop-Guardrail-Warning", header_val.parse().unwrap());
        response
    }
}

/// Build a 403 blocked response with the guard name.
pub fn blocked_response(outcome: &GuardrailsOutcome) -> Response {
    let guard_name = outcome.blocking_guard.as_deref().unwrap_or("unknown");

    // Find the blocking guard result to get details
    let details = outcome
        .results
        .iter()
        .find(|r| match r {
            GuardResult::Failed { name, .. } => name == guard_name,
            GuardResult::Error { name, .. } => name == guard_name,
            _ => false,
        })
        .and_then(|r| match r {
            GuardResult::Failed { result, .. } => Some(json!({
                "evaluation_result": result,
                "reason": "evaluation_failed"
            })),
            GuardResult::Error { error, .. } => Some(json!({
                "error_details": error,
                "reason": "evaluator_error"
            })),
            _ => None,
        });

    let mut error_obj = json!({
        "type": "guardrail_blocked",
        "guardrail": guard_name,
        "message": format!("Request blocked by guardrail '{guard_name}'"),
    });

    if let Some(details) = details {
        if let Some(obj) = error_obj.as_object_mut() {
            if let Some(details_obj) = details.as_object() {
                obj.extend(details_obj.clone());
            }
        }
    }

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
