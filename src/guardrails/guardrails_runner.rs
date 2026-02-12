use std::collections::HashSet;

use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use tracing::warn;

use super::api_control::{parse_guardrails_header, resolve_guards_by_name, split_guards_by_mode};
use super::executor::execute_guards;
use super::input_extractor::{PromptExtractor, CompletionExtractor};
use super::types::{Guard, GuardWarning, GuardrailClient, Guardrails};

/// Result of running pre-call or post-call guards.
pub struct GuardPhaseResult {
    pub blocked_response: Option<Response>,
    pub warnings: Vec<GuardWarning>,
}

/// Runs guardrails across pre-call and post-call phases.
/// Shared between chat_completions and completions handlers.
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
        let input = response.extract_completion();
        let outcome = execute_guards(&self.post_call, &input, self.client).await;
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
        response.headers_mut().insert(
            "X-Traceloop-Guardrail-Warning",
            header_val.parse().unwrap(),
        );
        response
    }
}

/// Build a 403 blocked response with the guard name.
pub fn blocked_response(outcome: &super::types::GuardrailsOutcome) -> Response {
    use super::types::GuardResult;

    let guard_name = outcome.blocking_guard.as_deref().unwrap_or("unknown");

    // Find the blocking guard result to get details
    let details = outcome.results.iter()
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
        .map(|w| format!("guardrail_name=\"{}\", reason=\"{}\"", w.guard_name, w.reason))
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
    let resolved = resolve_guards_by_name(&gr.all_guards, &pipeline_names, &header_names, &[]);

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
