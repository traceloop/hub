use crate::pipelines::plugin::Plugin;
use crate::config::models::PluginConfig;
use crate::chat::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::state::AppState;
use std::sync::Arc;

pub struct ModelRouterPlugin {
    models: Vec<String>,
}

impl Plugin for ModelRouterPlugin {
    fn name(&self) -> String {
        "model-router".to_string()
    }

    fn enabled(&self) -> bool {
        true
    }

    fn init(&mut self, config: &PluginConfig) -> () {
        if let PluginConfig::ModelRouter { model_router } = config {
            self.models = model_router.models.clone();
        }
    }

    fn clone_box(&self) -> Box<dyn Plugin> {
        Box::new(ModelRouterPlugin {
            models: self.models.clone(),
        })
    }
}

impl ModelRouterPlugin {
    pub fn new(models: Vec<String>) -> Self {
        Self { models }
    }

    pub async fn handle_request(
        state: Arc<AppState>,
        request: ChatCompletionRequest,
    ) -> ChatCompletionResponse {
        // Try to find a model that matches the requested type
        for model_name in &state.config.models {
            if let Some(model) = state.model_registry.get(&model_name.name) {
                return model.chat_completions(state.clone(), request).await;
            }
        }

        panic!("No suitable model found for the request");
    }
} 