pub mod traceloop;

use self::traceloop::TraceloopClient;
use super::types::{Guard, GuardrailClient};

pub const TRACELOOP_PROVIDER: &str = "traceloop";

/// Create a guardrail client based on the guard's provider type.
pub fn create_guardrail_client(guard: &Guard) -> Option<Box<dyn GuardrailClient>> {
    match guard.provider.as_str() {
        TRACELOOP_PROVIDER => Some(Box::new(TraceloopClient::new())),
        _ => None,
    }
}
