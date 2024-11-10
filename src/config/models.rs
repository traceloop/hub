use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub providers: Vec<Provider>,
    pub models: Vec<ModelConfig>,
    pub pipelines: Vec<Pipeline>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Provider {
    pub key: String,
    pub r#type: String,
    pub api_key: String,
    #[serde(flatten)]
    pub params: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelConfig {
    pub key: String,
    pub r#type: String,
    pub provider: String,
    #[serde(flatten)]
    pub params: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PipelineType {
    Chat,
    Completion,
    Embeddings,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pipeline {
    pub name: String,
    pub r#type: PipelineType,
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum PluginConfig {
    Logging {
        logging: LoggingConfig,
    },
    Tracing {
        tracing: TracingConfig,
    },
    ModelRouter {
        #[serde(rename = "model-router")]
        model_router: ModelRouterConfig,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub level: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TracingConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelRouterConfig {
    pub models: Vec<String>,
}
