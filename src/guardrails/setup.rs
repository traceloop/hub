use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::types::{
    Guard, GuardMode, GuardrailClient, GuardrailResources, Guardrails, GuardrailsConfig,
};

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

    let all_names = pipeline_names.iter().chain(header_names.iter()).copied();

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

/// Resolve provider defaults (api_base/api_key) for all guards in the config.
pub fn resolve_guard_defaults(config: &GuardrailsConfig) -> Vec<Guard> {
    let mut guards = config.guards.clone();
    for guard in &mut guards {
        if guard.api_base.is_none() || guard.api_key.is_none() {
            if let Some(provider) = config.providers.get(&guard.provider) {
                if guard.api_base.is_none() && !provider.api_base.is_empty() {
                    guard.api_base = Some(provider.api_base.clone());
                }
                if guard.api_key.is_none() && !provider.api_key.is_empty() {
                    guard.api_key = Some(provider.api_key.clone());
                }
            }
        }
    }
    guards
}

/// Build the shared guardrail resources (resolved guards + client).
/// Returns None if the config has no guards.
/// Called once per router build; the result is shared across all pipelines.
pub fn build_guardrail_resources(config: &GuardrailsConfig) -> Option<GuardrailResources> {
    if config.guards.is_empty() {
        return None;
    }
    let all_guards = Arc::new(resolve_guard_defaults(config));
    let client: Arc<dyn GuardrailClient> =
        Arc::new(super::providers::traceloop::TraceloopClient::new());
    Some((all_guards, client))
}

/// Build per-pipeline Guardrails from shared resources.
/// `shared` contains the Arc-wrapped guards and client built once by `build_guardrail_resources`.
pub fn build_pipeline_guardrails(
    shared: &GuardrailResources,
    pipeline_guard_names: &[String],
) -> Arc<Guardrails> {
    Arc::new(Guardrails {
        all_guards: shared.0.clone(),
        pipeline_guard_names: pipeline_guard_names.to_vec(),
        client: shared.1.clone(),
    })
}
