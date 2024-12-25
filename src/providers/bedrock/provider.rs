use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::{config::Region, Client as BedrockRuntimeClient};
use axum::async_trait;
use axum::http::StatusCode;

use super::models::{
    BedrockRequestPayload, BedrockResponsePayload, ClaudeRequest, ClaudeResponse, JurassicRequest,
    JurassicResponse, TitanEmbeddingsRequest, TitanEmbeddingsResponse, TitanRequest, TitanResponse,
};
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;
use aws_config::profile::profile_file::{ProfileFileKind, ProfileFiles};
use aws_sdk_bedrockruntime::primitives::Blob;
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::completion::CompletionChoice;

pub struct BedrockProvider {
    config: ProviderConfig,
    client: BedrockRuntimeClient,
}

impl BedrockProvider {
    async fn invoke_model<Req, Resp>(
        &self,
        model_id: &str,
        request: Req,
    ) -> Result<Resp, StatusCode>
    where
        Req: BedrockRequestPayload,
        Resp: BedrockResponsePayload,
    {
        let request_body = serde_json::to_vec(&request).map_err(|e| {
            eprintln!("Failed to serialize request: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let result = self
            .client
            .invoke_model()
            .content_type("application/json")
            .accept("application/json")
            .model_id(model_id)
            .body(Blob::new(request_body))
            .send()
            .await
            .map_err(|e| {
                eprintln!("Bedrock API error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let response_str = String::from_utf8(result.body.into_inner())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        serde_json::from_str(&response_str).map_err(|e| {
            eprintln!("Failed to deserialize response: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }
}

#[async_trait]
impl Provider for BedrockProvider {
    async fn new(config: &ProviderConfig) -> Self {
        let region = Region::new(
            config
                .params
                .get("region")
                .cloned()
                .unwrap_or_else(|| "us-east-1".to_string()),
        );

        let credentials_path = config
            .params
            .get("credentials_path")
            .expect("credentials_path is required");

        let profile_files = ProfileFiles::builder()
            .with_file(ProfileFileKind::Credentials, credentials_path)
            .build();

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .profile_files(profile_files)
            .region(region)
            .load()
            .await;

        let client = BedrockRuntimeClient::new(&sdk_config);

        Self {
            config: config.clone(),
            client,
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
        let model_type = &model_config.r#type;
        
        let response = match model_type {
            model if model.starts_with("anthropic.claude") => {
                let request = ClaudeRequest::from_chat_request(&payload);
                let response: ClaudeResponse = self.invoke_model(model_type, request).await?;
                response.into_chat_completion(model_type.clone())
            }
            model if model.starts_with("amazon.titan") => {
                let request = TitanRequest::from_chat_request(&payload);
                let response: TitanResponse = self.invoke_model(model_type, request).await?;
                response.into_chat_completion(model_type.clone())
            }
            model if model.starts_with("ai21.j2") => {
                let request = JurassicRequest::from_chat_request(&payload);
                let response: JurassicResponse = self.invoke_model(model_type, request).await?;
                response.into_chat_completion(model_type.clone())
            }
            _ => {
                return Err(StatusCode::NOT_IMPLEMENTED);
            }
        };

        Ok(ChatCompletionResponse::NonStream(response))
    }

    async fn completions(
        &self,
        payload: CompletionRequest,
        model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        let chat_request = ChatCompletionRequest {
            model: payload.model,
            messages: vec![ChatCompletionMessage {
                role: "user".to_string(),
                content: Some(ChatMessageContent::String(payload.prompt)),
                name: None,
                tool_calls: None,
            }],
            temperature: payload.temperature,
            top_p: payload.top_p,
            n: payload.n,
            stream: payload.stream,
            stop: payload.stop,
            max_tokens: payload.max_tokens,
            presence_penalty: payload.presence_penalty,
            frequency_penalty: payload.frequency_penalty,
            logit_bias: payload.logit_bias,
            user: payload.user,
            parallel_tool_calls: None,
            tool_choice: None,
            tools: None,
        };

        let chat_response = self.chat_completions(chat_request, model_config).await?;

        match chat_response {
            ChatCompletionResponse::NonStream(completion) => {
                let choice = completion.choices.first().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
                
                Ok(CompletionResponse {
                    id: completion.id,
                    object: "text_completion".to_string(),
                    created: completion.created.unwrap_or_default(),
                    model: completion.model,
                    choices: vec![CompletionChoice {
                        text: match &choice.message.content {
                            Some(ChatMessageContent::String(text)) => text.clone(),
                            _ => String::new(),
                        },
                        index: choice.index,
                        logprobs: None,
                        finish_reason: choice.finish_reason.clone(),
                    }],
                    usage: completion.usage,
                })
            }
            ChatCompletionResponse::Stream(_) => Err(StatusCode::NOT_IMPLEMENTED),
        }
    }

    async fn embeddings(
        &self,
        payload: EmbeddingsRequest,
        model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        let model_type = &model_config.r#type;

        // since titen model only support embedding
        if !model_type.starts_with("amazon.titan-embed") {
            return Err(StatusCode::NOT_IMPLEMENTED);
        }

        let request = TitanEmbeddingsRequest::from(payload);
        let result = self
            .client
            .invoke_model()
            .content_type("application/json")
            .accept("application/json")
            .model_id(model_type)
            .body(Blob::new(
                serde_json::to_string(&request)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .into_bytes(),
            ))
            .send()
            .await
            .map_err(|e| {
                eprintln!("Bedrock API error: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let response_str = String::from_utf8(result.body.into_inner())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let titan_response: TitanEmbeddingsResponse = serde_json::from_str(&response_str)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(titan_response.into_embeddings_response(model_type.clone()))
    }
}