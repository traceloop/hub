use futures::future::join_all;
use tracing::{debug, warn};

use super::providers::GuardrailClient;
use super::types::{Guard, GuardResult, GuardWarning, GuardrailsOutcome, OnFailure};

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
                        result: response.result,
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
