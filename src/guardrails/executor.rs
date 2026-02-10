use super::providers::GuardrailClient;
use super::types::{GuardConfig, GuardrailsOutcome};

/// Execute a set of guardrails against the given input text.
/// Returns a GuardrailsOutcome with results, blocked status, and warnings.
pub async fn execute_guards(
    _guards: &[GuardConfig],
    _input: &str,
    _client: &dyn GuardrailClient,
) -> GuardrailsOutcome {
    todo!("Implement guard execution orchestration")
}
