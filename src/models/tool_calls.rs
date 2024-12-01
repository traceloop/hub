use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FunctionCall {
    pub arguments: String,
    pub name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ChatMessageToolCall {
    pub id: String,
    pub function: FunctionCall,
    #[serde(rename = "type")]
    pub r#type: String, // Using `function` as the only valid value
}
