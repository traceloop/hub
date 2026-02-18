use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use tower::{Layer, Service};
use tracing::debug;

use crate::models::chat::{ChatCompletion, ChatCompletionRequest};

use super::runner::GuardrailsRunner;
use super::types::Guardrails;

/// Maximum request/response body size to buffer (10 MB).
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Tower layer that applies guardrail checks around a service.
///
/// - **Pre-call guards** run before the inner service, inspecting the request body.
/// - **Post-call guards** run after the inner service, inspecting the response body.
/// - Streaming requests (`"stream": true`) bypass guardrails entirely.
#[derive(Clone)]
pub struct GuardrailsLayer {
    guardrails: Option<Arc<Guardrails>>,
}

impl GuardrailsLayer {
    pub fn new(guardrails: Option<Arc<Guardrails>>) -> Self {
        Self { guardrails }
    }
}

impl<S> Layer<S> for GuardrailsLayer {
    type Service = GuardrailsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GuardrailsMiddleware {
            inner,
            guardrails: self.guardrails.clone(),
        }
    }
}

/// Tower service that wraps an inner service with guardrail checks.
#[derive(Clone)]
pub struct GuardrailsMiddleware<S> {
    inner: S,
    guardrails: Option<Arc<Guardrails>>,
}

impl<S> Service<Request<Body>> for GuardrailsMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let guardrails = self.guardrails.clone();
        // Clone inner and swap so the clone is used in the future
        // (standard Tower pattern to satisfy borrow checker)
        let inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner);

        Box::pin(async move {
            // No guardrails configured — pass through without buffering
            let guardrails = match guardrails {
                Some(gr) => gr,
                None => return inner.call(request).await,
            };

            let (parts, body) = request.into_parts();

            // Buffer request body
            let bytes = match axum::body::to_bytes(body, MAX_BODY_SIZE).await {
                Ok(b) => b,
                Err(_) => {
                    debug!("Guardrails middleware: failed to buffer request body, passing through");
                    return Ok(axum::http::StatusCode::BAD_REQUEST.into_response());
                }
            };

            // Try to parse as ChatCompletionRequest
            let chat_request: ChatCompletionRequest = match serde_json::from_slice(&bytes) {
                Ok(r) => r,
                Err(_) => {
                    // Not a chat completion request — pass through unchanged
                    let request = Request::from_parts(parts, Body::from(bytes));
                    return inner.call(request).await;
                }
            };

            // Skip guardrails for streaming requests
            if chat_request.stream.unwrap_or(false) {
                debug!("Guardrails middleware: streaming request, skipping guardrails");
                let request = Request::from_parts(parts, Body::from(bytes));
                return inner.call(request).await;
            }

            // Resolve guards from pipeline config + request headers
            let runner = GuardrailsRunner::new(Some(&guardrails), &parts.headers, None);

            let runner = match runner {
                Some(r) => r,
                None => {
                    // No active guards for this request
                    let request = Request::from_parts(parts, Body::from(bytes));
                    return inner.call(request).await;
                }
            };

            // --- Pre-call guards ---
            let pre_result = runner.run_pre_call(&chat_request).await;
            if let Some(blocked) = pre_result.blocked_response {
                return Ok(blocked);
            }
            let mut all_warnings = pre_result.warnings;

            // --- Call inner service ---
            let request = Request::from_parts(parts, Body::from(bytes));
            let response = inner.call(request).await?;

            // --- Post-call guards ---
            let (resp_parts, resp_body) = response.into_parts();
            let resp_bytes = match axum::body::to_bytes(resp_body, MAX_BODY_SIZE).await {
                Ok(b) => b,
                Err(_) => {
                    debug!("Guardrails middleware: failed to buffer response body, skipping post-call");
                    let response = Response::from_parts(resp_parts, Body::empty());
                    return Ok(GuardrailsRunner::finalize_response(response, &all_warnings));
                }
            };

            if let Ok(completion) = serde_json::from_slice::<ChatCompletion>(&resp_bytes) {
                let post_result = runner.run_post_call(&completion).await;
                if let Some(blocked) = post_result.blocked_response {
                    return Ok(blocked);
                }
                all_warnings.extend(post_result.warnings);
            }

            // Reconstruct response with original bytes and attach warning headers
            let response = Response::from_parts(resp_parts, Body::from(resp_bytes));
            Ok(GuardrailsRunner::finalize_response(response, &all_warnings))
        })
    }
}
