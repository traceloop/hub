use serde::de::{Deserializer, Error};
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
    #[serde(deserialize_with = "deserialize_embedding")]
    pub embedding: Embedding,
    pub index: usize,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum Embedding {
    String(String),
    Float(Vec<f32>),
    Json(Value),
}

// Custom deserializer for Embedding to handle various formats
fn deserialize_embedding<'de, D>(deserializer: D) -> Result<Embedding, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;

    match value {
        // If it's a string, use the String variant
        Value::String(s) => Ok(Embedding::String(s)),

        // If it's an array, convert to Vec<f32>
        Value::Array(arr) => {
            let arr_clone = arr.clone();
            let floats: Result<Vec<f32>, _> = arr
                .into_iter()
                .map(|v| match v {
                    Value::Number(n) => n
                        .as_f64()
                        .ok_or_else(|| D::Error::custom("Expected float value"))
                        .map(|f| f as f32),
                    _ => Err(D::Error::custom("Expected number in array")),
                })
                .collect();

            match floats {
                Ok(float_vec) => Ok(Embedding::Float(float_vec)),
                Err(_) => Ok(Embedding::Json(Value::Array(arr_clone))), // Fallback to JSON if conversion fails
            }
        }

        // For any other JSON value, store it as is
        _ => Ok(Embedding::Json(value)),
    }
}
