pub mod traceloop;

use self::traceloop::TraceloopClient;
use super::types::{Guard, GuardrailClient};

/// Create a guardrail client based on the guard's provider type.
pub fn create_guardrail_client(guard: &Guard) -> Option<Box<dyn GuardrailClient>> {
    match guard.provider.as_str() {
        "traceloop" => Some(Box::new(TraceloopClient::new())),
        _ => None,
    }
}
