use async_trait::async_trait;
use std::time::Duration;

use super::GuardrailClient;
use crate::guardrails::types::{EvaluatorResponse, GuardConfig, GuardrailError};

/// HTTP client for the Traceloop evaluator API service.
/// Calls `POST {api_base}/v2/guardrails/{evaluator_slug}`.
pub struct TraceloopClient {
    http_client: reqwest::Client,
}

impl TraceloopClient {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            http_client: reqwest::Client::builder().timeout(timeout).build().unwrap(),
        }
    }
}

#[async_trait]
impl GuardrailClient for TraceloopClient {
    async fn evaluate(
        &self,
        _guard: &GuardConfig,
        _input: &str,
    ) -> Result<EvaluatorResponse, GuardrailError> {
        todo!("Implement Traceloop evaluator API call")
    }
}
