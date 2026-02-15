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

/// Resolve the final set of guards to execute by merging pipeline and header sources.
pub fn resolve_guards_by_name(
    all_guards: &[Guard],
    pipeline_names: &[&str],
    header_names: &[&str],
) -> Vec<Guard> {
    let guard_map: HashMap<&str, &Guard> =
        all_guards.iter().map(|g| (g.name.as_str(), g)).collect();

    let mut seen = HashSet::new();
    let mut resolved = Vec::new();

    let all_names = pipeline_names
        .iter()
        .chain(header_names.iter())
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
