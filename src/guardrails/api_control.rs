use super::types::{GuardConfig, GuardMode};

/// Parse guard names from the X-Traceloop-Guardrails header value.
/// Names are comma-separated and trimmed.
pub fn parse_guardrails_header(_header: &str) -> Vec<String> {
    todo!("Implement header parsing")
}

/// Parse guard names from the request payload's `guardrails` field.
pub fn parse_guardrails_from_payload(_payload: &serde_json::Value) -> Vec<String> {
    todo!("Implement payload guardrails parsing")
}

/// Resolve the final set of guards to execute by merging pipeline, header, and payload sources.
/// Guards are additive and deduplicated by name.
pub fn resolve_guards_by_name<'a>(
    _all_guards: &'a [GuardConfig],
    _pipeline_names: &[&str],
    _header_names: &[&str],
    _payload_names: &[&str],
) -> Vec<GuardConfig> {
    todo!("Implement additive guard resolution")
}

/// Split guards into (pre_call, post_call) lists by mode.
pub fn split_guards_by_mode(_guards: &[GuardConfig]) -> (Vec<GuardConfig>, Vec<GuardConfig>) {
    todo!("Implement guard splitting by mode")
}
