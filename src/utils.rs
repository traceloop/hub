use crate::models::ModelProvider;
use axum::http::HeaderMap;

pub fn extract_provider(headers: &HeaderMap) -> ModelProvider {
    match headers
        .get("x-traceloop-provider")
        .and_then(|h| h.to_str().ok())
    {
        Some("openai") => ModelProvider::OpenAI,
        Some("anthropic") => ModelProvider::Anthropic,
        _ => ModelProvider::Unknown,
    }
}
