use axum::async_trait;
use axum::http::StatusCode;
use std::error::Error;

use aws_sdk_bedrockruntime::Client as BedrockRuntimeClient;

use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;

use crate::providers::anthropic::{
    AnthropicChatCompletionRequest, AnthropicChatCompletionResponse,
};
use crate::providers::bedrock::models::{
    Ai21ChatCompletionRequest, Ai21ChatCompletionResponse, Ai21CompletionsRequest,
    Ai21CompletionsResponse, TitanChatCompletionRequest, TitanChatCompletionResponse,
    TitanEmbeddingRequest, TitanEmbeddingResponse,
};
use aws_sdk_bedrockruntime::primitives::Blob;

struct AI21Implementation;
struct TitanImplementation;
struct AnthropicImplementation;

pub struct BedrockProvider {
    pub(crate) config: ProviderConfig,
}

pub trait ClientProvider {
    async fn create_client(&self) -> Result<BedrockRuntimeClient, String>;
}

#[cfg(not(test))]
impl ClientProvider for BedrockProvider {
    async fn create_client(&self) -> Result<BedrockRuntimeClient, String> {
        use aws_config::BehaviorVersion;
        use aws_config::Region;
        use aws_credential_types::Credentials;

        let region = self.config.params.get("region").unwrap().clone();
        let use_iam_role = self
            .config
            .params
            .get("use_iam_role")
            .map_or("false", |s| &**s);

        let sdk_config = if use_iam_role.parse::<bool>().unwrap_or(false) {
            aws_config::defaults(BehaviorVersion::latest())
                .region(Region::new(region))
                .load()
                .await
        } else {
            let access_key_id = self.config.params.get("AWS_ACCESS_KEY_ID").unwrap().clone();
            let secret_access_key = self
                .config
                .params
                .get("AWS_SECRET_ACCESS_KEY")
                .unwrap()
                .clone();
            let session_token = self.config.params.get("AWS_SESSION_TOKEN").cloned();

            let credentials =
                Credentials::from_keys(access_key_id, secret_access_key, session_token);

            aws_config::defaults(BehaviorVersion::latest())
                .region(Region::new(region))
                .credentials_provider(credentials)
                .load()
                .await
        };

        Ok(BedrockRuntimeClient::new(&sdk_config))
    }
}

impl BedrockProvider {
    fn get_provider_implementation(
        &self,
        model_config: &ModelConfig,
    ) -> Box<dyn BedrockModelImplementation> {
        let bedrock_model_provider = model_config.params.get("model_provider").unwrap();

        let provider_implementation: Box<dyn BedrockModelImplementation> =
            match bedrock_model_provider.as_str() {
                "ai21" => Box::new(AI21Implementation),
                "titan" => Box::new(TitanImplementation),
                "anthropic" => Box::new(AnthropicImplementation),
                _ => panic!("Invalid bedrock model provider"),
            };

        provider_implementation
    }

    fn transform_model_identifier(&self, model: String, model_config: &ModelConfig) -> String {
        // Check if the model is already an ARN or inference profile ID
        if model.starts_with("arn:aws:bedrock:") || model.contains("inference-profile") {
            // Use the model identifier as-is for ARNs and inference profiles
            model
        } else {
            // Transform model name to include provider prefix for regular model IDs
            let model_provider = model_config.params.get("model_provider").unwrap();
            let inference_profile_id = self.config.params.get("inference_profile_id");
            let model_version = model_config
                .params
                .get("model_version")
                .map_or("v1:0", |s| &**s);

            if let Some(profile_id) = inference_profile_id {
                format!(
                    "{}.{}.{}-{}",
                    profile_id, model_provider, model, model_version
                )
            } else {
                format!("{}.{}-{}", model_provider, model, model_version)
            }
        }
    }
}

#[async_trait]
impl Provider for BedrockProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "bedrock".to_string()
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let client = self.create_client().await.map_err(|e| {
            eprintln!("Failed to create Bedrock client: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let mut transformed_payload = payload;

        transformed_payload.model =
            self.transform_model_identifier(transformed_payload.model, model_config);

        self.get_provider_implementation(model_config)
            .chat_completion(&client, transformed_payload)
            .await
    }

    async fn completions(
        &self,
        payload: CompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        let client = self.create_client().await.map_err(|e| {
            eprintln!("Failed to create Bedrock client: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let mut transformed_payload = payload;

        transformed_payload.model =
            self.transform_model_identifier(transformed_payload.model, model_config);

        self.get_provider_implementation(model_config)
            .completion(&client, transformed_payload)
            .await
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let client = self.create_client().await.map_err(|e| {
            eprintln!("Failed to create Bedrock client: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let mut transformed_payload = payload;

        transformed_payload.model =
            self.transform_model_identifier(transformed_payload.model, model_config);

        self.get_provider_implementation(model_config)
            .embedding(&client, transformed_payload)
            .await
    }
}

/**
        BEDROCK IMPLEMENTATION TEMPLATE - WILL SERVE AS LAYOUT FOR OTHER IMPLEMENTATIONS
*/

#[async_trait]
trait BedrockModelImplementation: Send + Sync {
    async fn chat_completion(
        &self,
        client: &BedrockRuntimeClient,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode>;

    async fn completion(
        &self,
        _client: &BedrockRuntimeClient,
        _payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode> {
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn embedding(
        &self,
        _client: &BedrockRuntimeClient,
        _payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        Err(StatusCode::NOT_IMPLEMENTED)
    }
}

trait BedrockRequestHandler {
    async fn handle_bedrock_request<T, U>(
        &self,
        client: &BedrockRuntimeClient,
        model_id: &str,
        request: T,
        error_context: &str,
    ) -> Result<U, StatusCode>
    where
        T: serde::Serialize + std::marker::Send,
        U: for<'de> serde::Deserialize<'de>,
    {
        // Serialize request
        let request_json = serde_json::to_vec(&request).map_err(|e| {
            eprintln!("Failed to serialize {}: {}", error_context, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Make API call
        let response = client
            .invoke_model()
            .body(Blob::new(request_json))
            .model_id(model_id)
            .send()
            .await
            .map_err(|e| {
                eprintln!("Bedrock API error for {}: {:?}", error_context, e);
                eprintln!(
                    "Error details - Source: {}, Raw error: {:?}",
                    e.source().unwrap_or(&e),
                    e.raw_response()
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Deserialize response
        serde_json::from_slice(&response.body.into_inner()).map_err(|e| {
            eprintln!("Failed to deserialize {} response: {}", error_context, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }
}

impl BedrockRequestHandler for AI21Implementation {}
impl BedrockRequestHandler for TitanImplementation {}
impl BedrockRequestHandler for AnthropicImplementation {}

/**
        AI21 IMPLEMENTATION
*/

#[async_trait]
impl BedrockModelImplementation for AI21Implementation {
    async fn chat_completion(
        &self,
        client: &BedrockRuntimeClient,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let ai21_request = Ai21ChatCompletionRequest::from(payload.clone());
        let ai21_response: Ai21ChatCompletionResponse = self
            .handle_bedrock_request(client, &payload.model, ai21_request, "AI21 chat completion")
            .await?;

        Ok(ChatCompletionResponse::NonStream(ai21_response.into()))
    }

    async fn completion(
        &self,
        client: &BedrockRuntimeClient,
        payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode> {
        // Bedrock AI21 supports completions in legacy models similar to openai
        let ai21_request = Ai21CompletionsRequest::from(payload.clone());
        let ai21_response: Ai21CompletionsResponse = self
            .handle_bedrock_request(client, &payload.model, ai21_request, "AI21 completion")
            .await?;

        Ok(CompletionResponse::from(ai21_response))
    }
}

/**
        TITAN IMPLEMENTATION
*/

#[async_trait]
impl BedrockModelImplementation for TitanImplementation {
    async fn chat_completion(
        &self,
        client: &BedrockRuntimeClient,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let titan_request = TitanChatCompletionRequest::from(payload.clone());
        let titan_response: TitanChatCompletionResponse = self
            .handle_bedrock_request(
                client,
                &payload.model,
                titan_request,
                "Titan chat completion",
            )
            .await?;

        Ok(ChatCompletionResponse::NonStream(titan_response.into()))
    }

    async fn embedding(
        &self,
        client: &BedrockRuntimeClient,
        payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let titan_request = TitanEmbeddingRequest::from(payload.clone());
        let titan_response: TitanEmbeddingResponse = self
            .handle_bedrock_request(client, &payload.model, titan_request, "Titan embedding")
            .await?;

        Ok(EmbeddingsResponse::from(titan_response))
    }
}

/**
        ANTHROPIC IMPLEMENTATION
*/

#[async_trait]
impl BedrockModelImplementation for AnthropicImplementation {
    async fn chat_completion(
        &self,
        client: &BedrockRuntimeClient,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let anthropic_request = AnthropicChatCompletionRequest::from(payload.clone());

        // Convert to Value for Bedrock-specific modifications
        let mut request_value = serde_json::to_value(&anthropic_request).map_err(|e| {
            eprintln!("Failed to serialize Anthropic request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let serde_json::Value::Object(ref mut map) = request_value {
            map.remove("model");
            map.insert(
                "anthropic_version".to_string(),
                serde_json::Value::String("bedrock-2023-05-31".to_string()),
            );
        }

        let anthropic_response: AnthropicChatCompletionResponse = self
            .handle_bedrock_request(
                client,
                &payload.model,
                request_value,
                "Anthropic chat completion",
            )
            .await?;

        Ok(ChatCompletionResponse::NonStream(anthropic_response.into()))
    }
}
