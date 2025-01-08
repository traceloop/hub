use std::error::Error;
use axum::async_trait;
use axum::http::StatusCode;

use aws_sdk_bedrockruntime::Client as BedrockRuntimeClient;
use aws_config::BehaviorVersion;
use aws_config::Region;
use aws_credential_types::Credentials;


use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;

use crate::providers::anthropic::models::{
    AnthropicChatCompletionRequest,
    AnthropicChatCompletionResponse,
};

use aws_sdk_bedrockruntime::primitives::Blob;
use crate::providers::bedrock::models::{Ai21ChatCompletionRequest, Ai21ChatCompletionResponse, Ai21CompletionsRequest, Ai21CompletionsResponse, TitanChatCompletionRequest, TitanChatCompletionResponse, TitanEmbeddingRequest, TitanEmbeddingResponse};
// https://www.shuttle.dev/blog/2024/05/10/prompting-aws-bedrock-rust

// diff -> https://stackoverflow.com/questions/76192496/openai-v1-completions-vs-v1-chat-completions-end-points

/*
Support all major Bedrock models:
Anthropic Claude models
Amazon Titan models
AI21 Jurassic models - us east 1 not 2

    the lagacy versons seem to be different
    Jamba 1.5 Large  Jamba 1.5 Mini jamba-Instruct
Stability.ai models


Notes for me to remember:

Antropic models use the same format
   I can pass control to the antrhopic provider and it will pass correct
their models start with  "anthropic."
checked :
Claude 3.5 Haiku
Claude 3.5 Sonnet
Claude 3 Haiku



Amazon Titan model - it is an embedding

https://us-east-2.console.aws.amazon.com/bedrock/home?region=us-east-2#/model-catalog/serverless/amazon.titan-embed-text-v2:0

https://us-east-1.console.aws.amazon.com/bedrock/home?region=us-east-1#/model-catalog/serverless/amazon.titan-embed-text-v1
starts with "amazon.titan"

inputText


titan takes only one input text , not sure why enum has an option for multiple
    TODO : Ask them why ?


I will forward control to antripic if the chatcompletion is from antropic

note : chat completions accepts role with completion does not
 */


struct AI21Implementation;
struct TitanImplementation;
struct  AnthropicImplementation;



pub struct BedrockProvider {
    config: ProviderConfig,
    // client :  BedrockRuntimeClient
}


impl BedrockProvider {
    async fn create_client(&self) -> Result<BedrockRuntimeClient, String> {

        let region = self.config
            .params
            .get("region")
            .unwrap()
            .clone();

        let access_key_id = self.config
            .params
            .get("AWS_ACCESS_KEY_ID")
            .unwrap()
            .clone();

        let secret_access_key = self.config
            .params
            .get("AWS_SECRET_ACCESS_KEY")
            .unwrap()
            .clone();

        let session_token = self.config
            .params
            .get("AWS_SESSION_TOKEN")
            .cloned();

        //TODO : need to remember that session token is optional

        let credentials =Credentials::from_keys(
            access_key_id.clone(),
            secret_access_key.clone(),
            session_token,
        );

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(region.clone()))
            .credentials_provider(credentials)
            .load()
            .await;

        Ok(BedrockRuntimeClient::new(&sdk_config))
    }
}


#[async_trait]
impl Provider for BedrockProvider {
    fn new(config: &ProviderConfig) -> Self {

        // let client = BedrockProvider::create_client(config);

        Self {
            config: config.clone(),
            // client
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "bedrock".to_string()
    }

    async fn chat_completions(&self, payload: ChatCompletionRequest, model_config: &ModelConfig) -> Result<ChatCompletionResponse, StatusCode> {

        let client = self.create_client().await.map_err(|e| {
            eprintln!("Failed to create Bedrock client: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // AI21Implementation.chat_completion(&client, payload).await

        TitanImplementation.chat_completion(&client, payload).await

        // // ANTROPIC WORKS FINE - IGNORE
        //
        // let anthropic_request = AnthropicChatCompletionRequest::from(payload.clone());
        //
        // // Convert to Value for modification
        // let mut request_value = serde_json::to_value(&anthropic_request).map_err(|e| {
        //     eprintln!("Failed to serialize request to value: {}", e);
        //     StatusCode::INTERNAL_SERVER_ERROR
        // })?;
        //
        // // Modify the JSON structure for Bedrock
        // if let serde_json::Value::Object(ref mut map) = request_value {
        //     map.remove("model"); // Remove model field
        //     map.insert("anthropic_version".to_string(),
        //                serde_json::Value::String("bedrock-2023-05-31".to_string()));
        // }
        //
        // // Print the modified JSON for debugging
        // println!("Debug - Modified Request JSON for Bedrock:\n{}",
        //          serde_json::to_string_pretty(&request_value).unwrap_or_default());
        //
        // // Convert to bytes for the actual request
        // let request_json = serde_json::to_vec(&request_value).map_err(|e| {
        //     eprintln!("Failed to serialize final request: {}", e);
        //     StatusCode::INTERNAL_SERVER_ERROR
        // })?;
        //
        // let res = client
        //     .invoke_model()
        //     .body(Blob::new(request_json))
        //     .model_id(&payload.model)
        //     .send()
        //     .await
        //     .map_err(|e| {
        //         eprintln!("Bedrock API request error: {:?}", e);  // Using {:?} debug formatter
        //         eprintln!("Error details - Source: {}, Raw error: {:?}", e.source().unwrap_or(&e), e.raw_response());
        //         StatusCode::INTERNAL_SERVER_ERROR
        //     })?;
        //
        // let anthropic_response: AnthropicChatCompletionResponse =
        //     serde_json::from_slice(&res.body.into_inner()).map_err(|e| {
        //         eprintln!("Failed to deserialize response: {}", e);
        //         StatusCode::INTERNAL_SERVER_ERROR
        //     })?;
        //
        //
        // println!("dev:now : Successfully processed chat completion");
        //
        // Ok(ChatCompletionResponse::NonStream(anthropic_response.into()))


    }

    async fn completions(&self, payload: CompletionRequest, _model_config: &ModelConfig) -> Result<CompletionResponse, StatusCode> {

        let client = self.create_client().await.map_err(|e| {
            eprintln!("Failed to create Bedrock client: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        AI21Implementation.completion(&client, payload).await
    }

    async fn embeddings(&self, payload: EmbeddingsRequest, _model_config: &ModelConfig) -> Result<EmbeddingsResponse, StatusCode> {


        let client = self.create_client().await.map_err(|e| {
            eprintln!("Failed to create Bedrock client: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        TitanImplementation.embedding(&client, payload).await

    }
}


/**
        BEDROCK IMPLEMENTATION TEMPLATE - WILL SERVE AS LAYOUT FOR OTHER IMPLEMENTATIONS
*/

#[async_trait]
trait BedrockModelImplementation: Send + Sync {
    /// Required method for chat completions - all models must implement this
    async fn chat_completion(
        &self,
        client: &BedrockRuntimeClient,
        payload: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, StatusCode>;

    /// Optional method for completions - default returns NOT_IMPLEMENTED
    /// Only AI21 currently implements this
    async fn completion(
        &self,
        client: &BedrockRuntimeClient,
        payload: CompletionRequest,
    ) -> Result<CompletionResponse, StatusCode> {
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    /// Optional method for embeddings - default returns NOT_IMPLEMENTED
    /// Only Titan currently implements this
    async fn embedding(
        &self,
        client: &BedrockRuntimeClient,
        payload: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    /// Helper method to handle common error cases with Bedrock client calls
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
                eprintln!("Error details - Source: {}, Raw error: {:?}",
                          e.source().unwrap_or(&e), e.raw_response());
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Deserialize response
        serde_json::from_slice(&response.body.into_inner()).map_err(|e| {
            eprintln!("Failed to deserialize {} response: {}", error_context, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }
}

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
            .handle_bedrock_request(
                client,
                &payload.model,
                ai21_request,
                "AI21 chat completion"
            )
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
            .handle_bedrock_request(
                client,
                &payload.model,
                ai21_request,
                "AI21 completion"
            )
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
                "Titan chat completion"
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
            .handle_bedrock_request(
                client,
                &payload.model,
                titan_request,
                "Titan embedding"
            )
            .await?;

        Ok(EmbeddingsResponse::from(titan_response))
        // https://us-east-2.console.aws.amazon.com/bedrock/home?region=us-east-2#/model-catalog/serverless/amazon.titan-embed-text-v2:0
    }
}

