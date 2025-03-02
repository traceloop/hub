use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::usage::EmbeddingUsage;

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
    SingleTokenIds(Vec<i32>),
    MultipleTokenIds(Vec<Vec<i32>>),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct EmbeddingsResponse {
    pub object: String,
    pub data: Vec<Embeddings>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Embeddings {
    pub object: String,
    pub embedding: Embedding,
    pub index: usize,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum Embedding {
    String(String),
    Float(Vec<f32>),
    Json(Value),
}
