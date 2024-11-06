use serde::{Deserialize, Serialize};

use super::common::Usage;

#[derive(Deserialize, Serialize, Clone)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub input: EmbeddingsInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum EmbeddingsInput {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct EmbeddingsResponse {
    pub object: String,
    pub data: Vec<Embeddings>,
    pub model: String,
    pub usage: Usage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Embeddings {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: usize,
}