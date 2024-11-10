use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::models::Provider as ProviderConfig;
use crate::providers::{
    anthropic::AnthropicProvider, azure::AzureProvider, openai::OpenAIProvider, provider::Provider,
};

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new(provider_configs: &[ProviderConfig]) -> Result<Self> {
        let mut providers = HashMap::new();

        for config in provider_configs {
            let provider: Arc<dyn Provider> = match config.r#type.as_str() {
                "openai" => Arc::new(OpenAIProvider::new(config)),
                "anthropic" => Arc::new(AnthropicProvider::new(config)),
                "azure" => Arc::new(AzureProvider::new(config)),
                _ => continue,
            };
            providers.insert(config.key.clone(), provider);
        }

        Ok(Self { providers })
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }
}
