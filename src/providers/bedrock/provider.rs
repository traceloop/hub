use axum::async_trait;
use axum::http::StatusCode;
use reqwest::Client;

use aws_sdk_bedrockruntime::Client as BedrockRuntimeClient;
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;

use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;

// https://www.shuttle.dev/blog/2024/05/10/prompting-aws-bedrock-rust
pub struct BedrockProvider {
    config: ProviderConfig,
    client :  BedrockRuntimeClient
}


impl BedrockProvider {
    async fn create_client(config: &ProviderConfig) -> Result<BedrockRuntimeClient, String> {

        let region = config
            .params
            .get("region")
            .clone();

        let access_key_id = config
            .get("AWS_ACCESS_KEY_ID")
            .clone();

        let secret_access_key = config
            .get("AWS_SECRET_ACCESS_KEY")
            .clone();

        let session_token = config
            .get("AWS_SESSION_TOKEN")
            .clone();

        //TODO : need to remember that session token is optional

        let credentials =Credentials::from_keys(
            access_key_id,
            secret_access_key,
            session_token,
        );

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .credentials_provider(credentials)
            .load()
            .await;

        Ok(BedrockRuntimeClient::new(&sdk_config))
    }
}


#[async_trait]
impl Provider for BedrockProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            config: config.clone(),
            client: BedrockProvider::create_client(config).unwrap()
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "bedrock".to_string()
    }

    async fn chat_completions(&self, payload: ChatCompletionRequest, model_config: &ModelConfig) -> Result<ChatCompletionResponse, StatusCode> {
        todo!()
    }

    async fn completions(&self, payload: CompletionRequest, model_config: &ModelConfig) -> Result<CompletionResponse, StatusCode> {
        todo!()
    }

    async fn embeddings(&self, payload: EmbeddingsRequest, model_config: &ModelConfig) -> Result<EmbeddingsResponse, StatusCode> {
        todo!()
    }
}