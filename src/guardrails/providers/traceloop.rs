use async_trait::async_trait;
use serde_json::json;
use std::time::Duration;

use super::GuardrailClient;
use crate::guardrails::response_parser::parse_evaluator_http_response;
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
        guard: &GuardConfig,
        input: &str,
    ) -> Result<EvaluatorResponse, GuardrailError> {
        let api_base = guard.api_base.as_deref().unwrap_or("http://localhost:8080");
        let url = format!(
            "{}/v2/guardrails/{}",
            api_base.trim_end_matches('/'),
            guard.evaluator_slug
        );

        let api_key = guard.api_key.as_deref().unwrap_or("");

        // Build config from params (excluding evaluator_slug which is top-level)
        let config: serde_json::Value = guard.params.clone().into_iter().collect();

        let body = json!({
            "inputs": [input],
            "config": config,
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    GuardrailError::Timeout(e.to_string())
                } else {
                    GuardrailError::Unavailable(e.to_string())
                }
            })?;

        let status = response.status().as_u16();
        let response_body = response
            .text()
            .await
            .map_err(|e| GuardrailError::Unavailable(e.to_string()))?;

        parse_evaluator_http_response(status, &response_body)
    }
}
