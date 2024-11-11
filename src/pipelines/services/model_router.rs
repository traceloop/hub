use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use axum::extract::Request;
use tower::Service;
use crate::state::AppState;

#[derive(Clone)]
pub struct ModelRouterService {
    state: Arc<AppState>,
    models: Vec<String>,
}

impl ModelRouterService {
    pub fn new(state: Arc<AppState>, models: Vec<String>) -> Self {
        Self { state, models }
    }
}

impl Service<Request> for ModelRouterService {
    type Response = axum::http::Response<String>;
    
    type Error = Infallible;
    
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        todo!()
    }
    
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}
