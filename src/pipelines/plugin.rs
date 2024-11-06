use crate::config::models::PluginConfig;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

pub trait Plugin {
    fn name(&self) -> String;
    fn enabled(&self) -> bool;
    fn init(&mut self, config: &PluginConfig);
    fn clone_box(&self) -> Box<dyn Plugin>;
}

impl Clone for Box<dyn Plugin> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub struct PluginMiddleware<S> {
    inner: S,
    plugin: Box<dyn Plugin>,
}

impl<S, Request> Service<Request> for PluginMiddleware<S>
where
    S: Service<Request>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        if !self.plugin.enabled() {
            let future = self.inner.call(request);
            return Box::pin(async move { future.await });
        }

        let future = self.inner.call(request);

        Box::pin(async move {
            let response = future.await?;
            // Here you can add post-processing logic
            Ok(response)
        })
    }
}

pub struct PluginLayer {
    pub(crate) plugin: Box<dyn Plugin>,
}

impl<S> Layer<S> for PluginLayer {
    type Service = PluginMiddleware<S>;

    fn layer(&self, service: S) -> Self::Service {
        PluginMiddleware {
            inner: service,
            plugin: self.plugin.clone(),
        }
    }
}
