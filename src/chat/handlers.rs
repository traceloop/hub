use crate::{
    config::models::{PipelineType, PluginConfig},
    pipelines::{
        pipeline::select_pipeline,
        plugin::{Plugin, PluginLayer},
        plugins::{logging::LoggingPlugin, tracing::TracingPlugin, model_router::ModelRouterPlugin},
    },
    state::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;
use tower::{ServiceBuilder, Service};

use super::models::{ChatCompletionRequest, ChatCompletionResponse};

pub async fn completions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, StatusCode> {
    let pipeline = select_pipeline(&state.config.pipelines, PipelineType::Chat, &headers)
        .ok_or(StatusCode::BAD_REQUEST)?;

    let mut service = ServiceBuilder::new();
    for plugin_config in &pipeline.plugins {
        let plugin: Box<dyn Plugin> = match plugin_config {
            PluginConfig::Logging { logging } => Box::new(LoggingPlugin),
            PluginConfig::Tracing { tracing } => Box::new(TracingPlugin),
            PluginConfig::ModelRouter { model_router } => {
                Box::new(ModelRouterPlugin::new(model_router.models.clone()))
            }
            _ => return Err(StatusCode::BAD_REQUEST),
        };

        service.layer(PluginLayer { plugin });
    }

    Ok(Json(
        service
            .service_fn(|req| async move { ModelRouterPlugin::handle_request(state, req).await })
            .call(payload)
            .await
            .unwrap(),
    ))
}
