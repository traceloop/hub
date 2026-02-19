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
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::EmbeddingsRequest;
use crate::pipelines::otel::SharedTracer;

use serde::de::DeserializeOwned;

use super::parsing::PromptExtractor;
use super::runner::GuardrailsRunner;
use super::types::Guardrails;

/// Enum representing the endpoint type.
#[derive(Debug, Clone, Copy)]
enum EndpointType {
    Chat,
    Completion,
    Embeddings,
}

impl EndpointType {
    /// Determine endpoint type from request path.
    fn from_path(path: &str) -> Option<Self> {
        match path {
            p if p.ends_with("/chat/completions") => Some(Self::Chat),
            p if p.ends_with("/completions") => Some(Self::Completion),
            p if p.ends_with("/embeddings") => Some(Self::Embeddings),
            _ => None,
        }
    }
}

/// Enum representing the type of request being processed.
enum ParsedRequest {
    Chat(Box<ChatCompletionRequest>),
    Completion(Box<CompletionRequest>),
    Embeddings(Box<EmbeddingsRequest>),
}

impl ParsedRequest {
    /// Returns true if this is a streaming request.
    fn is_streaming(&self) -> bool {
        match self {
            ParsedRequest::Chat(req) => req.stream.unwrap_or(false),
            ParsedRequest::Completion(req) => req.stream.unwrap_or(false),
            ParsedRequest::Embeddings(_) => false,
        }
    }

    /// Returns true if this request type supports post-call guards.
    /// Streaming requests do not support post-call guards because the response
    /// is sent as chunks and cannot be buffered for evaluation.
    fn supports_post_call(&self) -> bool {
        if self.is_streaming() {
            return false;
        }
        match self {
            ParsedRequest::Chat(_) | ParsedRequest::Completion(_) => true,
            ParsedRequest::Embeddings(_) => false,
        }
    }
}

impl PromptExtractor for ParsedRequest {
    fn extract_prompt(&self) -> String {
        match self {
            ParsedRequest::Chat(req) => req.extract_prompt(),
            ParsedRequest::Completion(req) => req.extract_prompt(),
            ParsedRequest::Embeddings(req) => req.extract_prompt(),
        }
    }
}

/// Helper function to handle post-call guards for supported request types.
async fn handle_post_call_guards(
    parsed_request: &ParsedRequest,
    resp_parts: axum::http::response::Parts,
    resp_body: Body,
    runner: &GuardrailsRunner<'_>,
    mut warnings: Vec<super::types::GuardWarning>,
) -> Response {
    let resp_bytes = match axum::body::to_bytes(resp_body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => {
            debug!("Guardrails middleware: failed to buffer response body, skipping post-call");
            let response = Response::from_parts(resp_parts, Body::empty());
            return GuardrailsRunner::finalize_response(response, &warnings);
        }
    };

    let post_result = match parsed_request {
        ParsedRequest::Chat(_) => {
            if let Ok(completion) = serde_json::from_slice::<ChatCompletion>(&resp_bytes) {
                Some(runner.run_post_call(&completion).await)
            } else {
                debug!("Guardrails middleware: failed to parse chat completion response");
                None
            }
        }
        ParsedRequest::Completion(_) => {
            if let Ok(completion) = serde_json::from_slice::<CompletionResponse>(&resp_bytes) {
                Some(runner.run_post_call(&completion).await)
            } else {
                debug!("Guardrails middleware: failed to parse completion response");
                None
            }
        }
        ParsedRequest::Embeddings(_) => None,
    };

    if let Some(result) = post_result {
        match result {
            Err(blocked) => return *blocked,
            Ok(w) => warnings.extend(w),
        }
    }

    let response = Response::from_parts(resp_parts, Body::from(resp_bytes));
    GuardrailsRunner::finalize_response(response, &warnings)
}

/// Try to deserialize bytes into a request type, logging on failure.
/// Returns None if deserialization fails, allowing the caller to pass through.
fn try_parse<T: DeserializeOwned>(bytes: &[u8], label: &str) -> Option<T> {
    match serde_json::from_slice::<T>(bytes) {
        Ok(req) => Some(req),
        Err(e) => {
            debug!(
                "Guardrails middleware: failed to parse {} request: {}",
                label, e
            );
            None
        }
    }
}

/// Tower layer that applies guardrail checks around a service.
///
/// - **Pre-call guards** run before the inner service, inspecting the request body.
/// - **Post-call guards** run after the inner service, inspecting the response body.
/// - Streaming requests (`"stream": true`) run pre-call guards but skip post-call guards.
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

#[derive(Clone)]
pub struct GuardrailsMiddleware<S> {
    inner: S, // pipeline router
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
        let inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner);

        Box::pin(async move {
            // No guardrails configured — pass through without buffering
            let guardrails = match guardrails {
                Some(gr) => gr,
                None => return inner.call(request).await,
            };

            let (parts, body) = request.into_parts();

            // Determine endpoint type from path (more efficient than parsing JSON)
            let endpoint_type = match EndpointType::from_path(parts.uri.path()) {
                Some(t) => t,
                None => {
                    // Unsupported endpoint — pass through
                    debug!(
                        "Guardrails middleware: unsupported endpoint {}, passing through",
                        parts.uri.path()
                    );
                    let request = Request::from_parts(parts, body);
                    return inner.call(request).await;
                }
            };

            // Buffer request body
            let bytes = match axum::body::to_bytes(body, usize::MAX).await {
                Ok(b) => b,
                Err(_) => {
                    debug!("Guardrails middleware: failed to buffer request body, passing through");
                    return Ok(axum::http::StatusCode::BAD_REQUEST.into_response());
                }
            };

            // Parse request based on endpoint type
            let parsed_request = match endpoint_type {
                EndpointType::Chat => try_parse::<ChatCompletionRequest>(&bytes, "chat")
                    .map(|req| ParsedRequest::Chat(Box::new(req))),
                EndpointType::Completion => try_parse::<CompletionRequest>(&bytes, "completion")
                    .map(|req| ParsedRequest::Completion(Box::new(req))),
                EndpointType::Embeddings => try_parse::<EmbeddingsRequest>(&bytes, "embeddings")
                    .map(|req| ParsedRequest::Embeddings(Box::new(req))),
            };
            let parsed_request = match parsed_request {
                Some(pr) => pr,
                None => {
                    let request = Request::from_parts(parts, Body::from(bytes));
                    return inner.call(request).await;
                }
            };

            // Resolve guards from pipeline config + request headers
            // Extract parent context from the tracer in request extensions
            let parent_cx = parts
                .extensions
                .get::<SharedTracer>()
                .and_then(|tracer| tracer.lock().ok().map(|t| t.parent_context()));
            let runner = GuardrailsRunner::new(Some(&guardrails), &parts.headers, parent_cx);

            let runner = match runner {
                Some(r) => r,
                None => {
                    // No active guards for this request
                    let request = Request::from_parts(parts, Body::from(bytes));
                    return inner.call(request).await;
                }
            };

            // --- Pre-call guards ---
            let all_warnings = match runner.run_pre_call(&parsed_request).await {
                Ok(warnings) => warnings,
                Err(blocked) => return Ok(*blocked),
            };

            // --- Call inner service ---
            let request = Request::from_parts(parts, Body::from(bytes));
            let response = inner.call(request).await?;

            // --- Post-call guards (only for request types that produce text) ---
            let (resp_parts, resp_body) = response.into_parts();

            if parsed_request.supports_post_call() {
                Ok(handle_post_call_guards(
                    &parsed_request,
                    resp_parts,
                    resp_body,
                    &runner,
                    all_warnings,
                )
                .await)
            } else {
                // No post-call guards for this request type (e.g., embeddings)
                // Pass through response with pre-call warnings attached
                let response = Response::from_parts(resp_parts, resp_body);
                Ok(GuardrailsRunner::finalize_response(response, &all_warnings))
            }
        })
    }
}
