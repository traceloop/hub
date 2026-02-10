pub mod traceloop;

use async_trait::async_trait;

use super::types::{EvaluatorResponse, GuardConfig, GuardrailError};

/// Trait for guardrail evaluator clients.
/// Each provider (traceloop, etc.) implements this to call its evaluator API.
#[async_trait]
pub trait GuardrailClient: Send + Sync {
    async fn evaluate(
        &self,
        guard: &GuardConfig,
        input: &str,
    ) -> Result<EvaluatorResponse, GuardrailError>;
}

/// Create a guardrail client based on the guard's provider type.
pub fn create_guardrail_client(_guard: &GuardConfig) -> Option<Box<dyn GuardrailClient>> {
    todo!("Implement client factory based on guard.provider")
}
