use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Simple(SimpleToolChoice),
    Named(ChatCompletionNamedToolChoice),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimpleToolChoice {
    None,
    Auto,
    Required,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionNamedToolChoice {
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    pub function: Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
}
