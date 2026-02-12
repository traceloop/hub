use std::sync::Arc;

use super::types::{GuardrailResources, GuardrailsConfig, Guardrails};

/// Resolve provider defaults (api_base/api_key) for all guards in the config.
pub fn resolve_guard_defaults(config: &GuardrailsConfig) -> Vec<super::types::Guard> {
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
pub fn build_guardrail_resources(
    config: &GuardrailsConfig,
) -> Option<GuardrailResources> {
    if config.guards.is_empty() {
        return None;
    }
    let all_guards = Arc::new(resolve_guard_defaults(config));
    let client: Arc<dyn super::providers::GuardrailClient> =
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
