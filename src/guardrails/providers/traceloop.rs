use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

use super::GuardrailClient;
use crate::guardrails::evaluator_types::get_evaluator;
use crate::guardrails::parsing::parse_evaluator_http_response;
use crate::guardrails::types::{EvaluatorResponse, Guard, GuardrailError};

const DEFAULT_TRACELOOP_API: &str = "https://api.traceloop.com";
/// HTTP client for the Traceloop evaluator API service.
/// Calls `POST {api_base}/v2/guardrails/{evaluator_slug}`.
pub struct TraceloopClient {
    http_client: reqwest::Client,
}

impl Default for TraceloopClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceloopClient {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    pub fn with_timeout(timeout: std::time::Duration) -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl GuardrailClient for TraceloopClient {
    async fn evaluate(
        &self,
        guard: &Guard,
        input: &str,
    ) -> Result<EvaluatorResponse, GuardrailError> {
        let api_base = guard
            .api_base
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_TRACELOOP_API);
        let url = format!(
            "{}/v2/guardrails/execute/{}",
            api_base, guard.evaluator_slug
        );

        let api_key = guard.api_key.as_deref().unwrap_or("");

        let evaluator = get_evaluator(&guard.evaluator_slug).ok_or_else(|| {
            GuardrailError::Unavailable(format!(
                "Unknown evaluator slug '{}'",
                guard.evaluator_slug
            ))
        })?;
        let body = evaluator.build_body(input, &guard.params)?;

        debug!(guard = %guard.name, slug = %guard.evaluator_slug, %url, %body, "NOMI - Calling evaluator API");

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();
        let response_body = response.text().await?;

        debug!(guard = %guard.name, %status, %response_body, "RON - Evaluator API response");

        parse_evaluator_http_response(status, &response_body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_body_text_slug() {
        let params = HashMap::new();
        let body = get_evaluator("pii-detector")
            .unwrap()
            .build_body("hello world", &params)
            .unwrap();
        assert_eq!(body, json!({"input": {"text": "hello world"}}));
    }

    #[test]
    fn test_build_body_prompt_slug() {
        let params = HashMap::new();
        let body = get_evaluator("prompt-injection")
            .unwrap()
            .build_body("hello world", &params)
            .unwrap();
        assert_eq!(body, json!({"input": {"prompt": "hello world"}}));
    }

    #[test]
    fn test_build_body_with_config() {
        let mut params = HashMap::new();
        params.insert("threshold".to_string(), json!(0.8));
        let body = get_evaluator("toxicity-detector")
            .unwrap()
            .build_body("test", &params)
            .unwrap();
        assert_eq!(
            body,
            json!({"input": {"text": "test"}, "config": {"threshold": 0.8}})
        );
    }

    #[test]
    fn test_build_body_no_config_when_params_empty() {
        let params = HashMap::new();
        let body = get_evaluator("secrets-detector")
            .unwrap()
            .build_body("test", &params)
            .unwrap();
        assert!(body.get("config").is_none());
    }

    #[test]
    fn test_get_evaluator_unknown_slug() {
        assert!(get_evaluator("nonexistent").is_none());
    }

    #[test]
    fn test_build_body_rejects_invalid_config_type() {
        let mut params = HashMap::new();
        params.insert("threshold".to_string(), json!("not-a-number"));
        let result = get_evaluator("toxicity-detector")
            .unwrap()
            .build_body("test", &params);
        assert!(result.is_err());
    }
}
