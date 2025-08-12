use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::models::Provider as ProviderConfig;
use crate::providers::{
    anthropic::AnthropicProvider, azure::AzureProvider, bedrock::BedrockProvider,
    openai::OpenAIProvider, provider::Provider, vertexai::VertexAIProvider,
};
use crate::types::ProviderType;

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new(provider_configs: &[ProviderConfig]) -> Result<Self> {
        let mut providers = HashMap::new();

        for config in provider_configs {
            let provider: Arc<dyn Provider> = match config.r#type {
                ProviderType::OpenAI => Arc::new(OpenAIProvider::new(config)),
                ProviderType::Anthropic => Arc::new(AnthropicProvider::new(config)),
                ProviderType::Azure => Arc::new(AzureProvider::new(config)),
                ProviderType::Bedrock => Arc::new(BedrockProvider::new(config)),
                ProviderType::VertexAI => Arc::new(VertexAIProvider::new(config)),
            };
            providers.insert(config.key.clone(), provider);
        }

        Ok(Self { providers })
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }
}
