use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct LogProbs {
    pub content: Vec<LogProbContent>,
}

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct LogProbContent {
    pub token: String,
    pub logprob: f32,
    pub bytes: Vec<u8>,
    pub top_logprobs: Vec<TopLogprob>,
}

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct TopLogprob {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i32>>,
    pub logprob: f64,
}

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct ChatCompletionTokenLogprob {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<i32>>,
    pub logprob: f64,
    pub top_logprobs: Vec<TopLogprob>,
}

#[derive(Deserialize, Serialize, Clone, Debug, ToSchema)]
pub struct ChoiceLogprobs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ChatCompletionTokenLogprob>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Vec<ChatCompletionTokenLogprob>>,
}
