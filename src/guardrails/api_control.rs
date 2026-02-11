use std::collections::{HashMap, HashSet};

use super::types::{Guard, GuardMode};

/// Parse guard names from the X-Traceloop-Guardrails header value.
/// Names are comma-separated and trimmed.
pub fn parse_guardrails_header(header: &str) -> Vec<String> {
    header
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse guard names from the request payload's `guardrails` field.
pub fn parse_guardrails_from_payload(payload: &serde_json::Value) -> Vec<String> {
    payload
        .get("guardrails")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve the final set of guards to execute by merging pipeline, header, and payload sources.
/// Guards are additive and deduplicated by name. Uses HashMap for O(1) guard lookup.
pub fn resolve_guards_by_name(
    all_guards: &[Guard],
    pipeline_names: &[&str],
    header_names: &[&str],
    payload_names: &[&str],
) -> Vec<Guard> {
    let guard_map: HashMap<&str, &Guard> =
        all_guards.iter().map(|g| (g.name.as_str(), g)).collect();

    let mut seen = HashSet::new();
    let mut resolved = Vec::new();

    let all_names = pipeline_names
        .iter()
        .chain(header_names.iter())
        .chain(payload_names.iter())
        .copied();

    for name in all_names {
        if seen.insert(name) {
            if let Some(guard) = guard_map.get(name) {
                resolved.push((*guard).clone());
            }
        }
    }

    resolved
}

/// Split guards into (pre_call, post_call) lists by mode.
pub fn split_guards_by_mode(guards: &[Guard]) -> (Vec<Guard>, Vec<Guard>) {
    let pre_call: Vec<Guard> = guards
        .iter()
        .filter(|g| g.mode == GuardMode::PreCall)
        .cloned()
        .collect();
    let post_call: Vec<Guard> = guards
        .iter()
        .filter(|g| g.mode == GuardMode::PostCall)
        .cloned()
        .collect();
    (pre_call, post_call)
}
