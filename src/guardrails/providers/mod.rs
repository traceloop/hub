pub mod traceloop;

use async_trait::async_trait;

use self::traceloop::TraceloopClient;
use super::types::{EvaluatorResponse, Guard, GuardrailError};

/// Trait for guardrail evaluator clients.
/// Each provider (traceloop, etc.) implements this to call its evaluator API.
#[async_trait]
pub trait GuardrailClient: Send + Sync {
    async fn evaluate(
        &self,
        guard: &Guard,
        input: &str,
    ) -> Result<EvaluatorResponse, GuardrailError>;
}

/// Create a guardrail client based on the guard's provider type.
pub fn create_guardrail_client(guard: &Guard) -> Option<Box<dyn GuardrailClient>> {
    match guard.provider.as_str() {
        "traceloop" => Some(Box::new(TraceloopClient::new())),
        _ => None,
    }
}
