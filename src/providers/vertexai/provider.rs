use super::models::{VertexAIChatCompletionRequest, VertexAIChatCompletionResponse};
use crate::config::models::{ModelConfig, Provider as ProviderConfig};
use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::providers::provider::Provider;
use axum::async_trait;
use axum::http::StatusCode;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde_json::json;

pub struct VertexAIProvider {
    config: ProviderConfig,
    http_client: Client,
}

#[async_trait]
impl Provider for VertexAIProvider {
    fn new(config: &ProviderConfig) -> Self {
        Self {
            config: config.clone(),
            http_client: Client::new(),
        }
    }

    fn key(&self) -> String {
        self.config.key.clone()
    }

    fn r#type(&self) -> String {
        "vertexai".to_string()
    }

    async fn chat_completions(
        &self,
        payload: ChatCompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<ChatCompletionResponse, StatusCode> {
        let request: VertexAIChatCompletionRequest = payload.into();

        let project_id = self
            .config
            .params
            .get("project_id")
            .ok_or(StatusCode::BAD_REQUEST)?;
        let location = self
            .config
            .params
            .get("location")
            .ok_or(StatusCode::BAD_REQUEST)?;
        let model = request.model.clone();

        let url = format!(
             "https://{location}-aiplatform.googleapis.com/v1/projects/{project_id}/locations/{location}/publishers/google/models/{model}:predict",
            location = location,
            project_id = project_id,
             model = model
        );

        let mut headers = HeaderMap::new();

        if let Some(api_key) = Some(self.config.api_key.as_str()) {
            headers.insert("x-goog-api-key", HeaderValue::from_str(api_key).unwrap());
            headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        } else {
            let token = self
                .get_auth_token()
                .await
                .map_err(|_| StatusCode::UNAUTHORIZED)?;
            headers.insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
            );
            headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        }

        let response = self
            .http_client
            .post(url)
            .headers(headers)
            .json(&json!({
                "instances": [request]
            }))
            .send()
            .await
            .map_err(|e| {
                eprintln!("VertexAI API request error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let status = response.status();

        if status.is_success() {
            let vertex_response: VertexAIChatCompletionResponse =
                response.json().await.map_err(|e| {
                    eprintln!("VertexAI API response error: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            Ok(ChatCompletionResponse::NonStream(vertex_response.into()))
        } else {
            eprintln!(
                "VertexAI API request error: {}",
                response.text().await.unwrap()
            );
            Err(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }

    async fn completions(
        &self,
        _payload: CompletionRequest,
        _model_config: &ModelConfig,
    ) -> Result<CompletionResponse, StatusCode> {
        unimplemented!()
    }

    async fn embeddings(
        &self,
        _payload: EmbeddingsRequest,
        _model_config: &ModelConfig,
    ) -> Result<EmbeddingsResponse, StatusCode> {
        unimplemented!()
    }
}

impl VertexAIProvider {
    async fn get_auth_token(&self) -> Result<String, anyhow::Error> {
        if let Some(credentials_path) = self.config.params.get("credentials_path") {
            let sa_key: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(credentials_path)?)?;
    
            let token_uri = sa_key
                .get("token_uri")
                .and_then(serde_json::Value::as_str)
                .ok_or(anyhow::anyhow!("token_uri not found"))?;
    
            let client_email = sa_key
                .get("client_email")
                .and_then(serde_json::Value::as_str)
                .ok_or(anyhow::anyhow!("client_email not found"))?;
    
            let private_key = sa_key
                .get("private_key")
                .and_then(serde_json::Value::as_str)
                .ok_or(anyhow::anyhow!("private_key not found"))?;
    
            let now = chrono::Utc::now();
            let claims = json!({
                "iss": client_email,
                "sub": client_email,
                "aud": token_uri,
                "exp": (now + chrono::Duration::minutes(60)).timestamp(),
                "iat": now.timestamp()
            });
    
            let header = Header::new(Algorithm::RS256);
            let signing_key = EncodingKey::from_rsa_pem(private_key.as_bytes())
                .map_err(|e| anyhow::anyhow!("Error decoding private key: {}", e))?;
            let jwt = encode(&header, &claims, &signing_key)
                .map_err(|e| anyhow::anyhow!("Error signing JWT: {}", e))?;
    
            let client = reqwest::Client::new();
            let params = [
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ];
            
            let response = client
                .post(token_uri)
                .form(&params)
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;
    
            let token = response
                .get("access_token")
                .and_then(serde_json::Value::as_str)
                .ok_or(anyhow::anyhow!("access_token not found"))?;
    
            Ok(token.to_string())
        } else if !self.config.api_key.is_empty() {
            Ok(self.config.api_key.clone())
        } else {
            Err(anyhow::anyhow!("No credentials or API key provided"))
        }
    }
}

