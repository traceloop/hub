use std::collections::HashSet;

use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use tracing::warn;

use super::api_control::{parse_guardrails_header, resolve_guards_by_name, split_guards_by_mode};
use super::executor::execute_guards;
use super::providers::GuardrailClient;
use super::types::{Guard, GuardWarning, Guardrails};

/// Result of running pre-call or post-call guards.
pub struct GuardPhaseResult {
    pub blocked_response: Option<Response>,
    pub warnings: Vec<GuardWarning>,
}

/// Orchestrates guardrail execution across pre-call and post-call phases.
/// Shared between chat_completions and completions handlers.
pub struct GuardrailOrchestrator<'a> {
    pre_call: Vec<Guard>,
    post_call: Vec<Guard>,
    client: &'a dyn GuardrailClient,
}

impl<'a> GuardrailOrchestrator<'a> {
    /// Create an orchestrator by resolving guards from pipeline config + request headers.
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

    /// Run pre-call guards against the extracted input text.
    pub async fn run_pre_call(&self, input: &str) -> GuardPhaseResult {
        if self.pre_call.is_empty() {
            return GuardPhaseResult {
                blocked_response: None,
                warnings: Vec::new(),
            };
        }
        let outcome = execute_guards(&self.pre_call, input, self.client).await;
        if outcome.blocked {
            return GuardPhaseResult {
                blocked_response: Some(blocked_response(&outcome.blocking_guard)),
                warnings: Vec::new(),
            };
        }
        GuardPhaseResult {
            blocked_response: None,
            warnings: outcome.warnings,
        }
    }

    /// Run post-call guards against the LLM response text.
    pub async fn run_post_call(&self, response_text: &str) -> GuardPhaseResult {
        if self.post_call.is_empty() {
            return GuardPhaseResult {
                blocked_response: None,
                warnings: Vec::new(),
            };
        }
        let outcome = execute_guards(&self.post_call, response_text, self.client).await;
        if outcome.blocked {
            return GuardPhaseResult {
                blocked_response: Some(blocked_response(&outcome.blocking_guard)),
                warnings: Vec::new(),
            };
        }
        GuardPhaseResult {
            blocked_response: None,
            warnings: outcome.warnings,
        }
    }

    /// Returns true if post-call guards are configured for this request.
    pub fn has_post_call_guards(&self) -> bool {
        !self.post_call.is_empty()
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
pub fn blocked_response(blocking_guard: &Option<String>) -> Response {
    let guard_name = blocking_guard.as_deref().unwrap_or("unknown");
    let body = json!({
        "error": {
            "type": "guardrail_blocked",
            "guardrail": guard_name,
            "message": format!("Request blocked by guardrail '{guard_name}'"),
        }
    });
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
