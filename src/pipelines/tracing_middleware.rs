use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use std::sync::{Arc, Mutex};
use tower::{Layer, Service};

use super::otel::OtelTracer;

#[derive(Clone)]
pub struct TracingLayer;

impl TracingLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for TracingLayer {
    type Service = TracingMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct TracingMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for TracingMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Create root traceloop_hub span and wrap in Arc<Mutex<>>
            let tracer = Arc::new(Mutex::new(OtelTracer::start()));

            // Insert into request extensions for downstream middleware and handlers
            req.extensions_mut().insert(tracer.clone());

            // Call inner service
            let response = inner.call(req).await?;

            // Tracer will be finalized when the Arc is dropped
            Ok(response)
        })
    }
}
