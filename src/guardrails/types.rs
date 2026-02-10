use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

fn default_on_failure() -> OnFailure {
    OnFailure::Warn
}

fn default_required() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GuardMode {
    PreCall,
    PostCall,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OnFailure {
    Block,
    Warn,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderConfig {
    pub name: String,
    pub api_base: String,
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GuardConfig {
    pub name: String,
    pub provider: String,
    pub evaluator_slug: String,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
    pub mode: GuardMode,
    #[serde(default = "default_on_failure")]
    pub on_failure: OnFailure,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl Hash for GuardConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.provider.hash(state);
        self.evaluator_slug.hash(state);
        // Hash params by sorting keys and hashing serialized values
        let mut params_vec: Vec<_> = self.params.iter().collect();
        params_vec.sort_by_key(|(k, _)| (*k).clone());
        for (k, v) in params_vec {
            k.hash(state);
            v.to_string().hash(state);
        }
        self.mode.hash(state);
        self.on_failure.hash(state);
        self.required.hash(state);
        self.api_base.hash(state);
        self.api_key.hash(state);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct GuardrailsConfig {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub guards: Vec<GuardConfig>,
}

impl Hash for GuardrailsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.providers.hash(state);
        self.guards.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResponse {
    pub result: serde_json::Value,
    pub pass: bool,
}

#[derive(Debug, Clone)]
pub enum GuardResult {
    Passed {
        name: String,
        result: serde_json::Value,
    },
    Failed {
        name: String,
        result: serde_json::Value,
        on_failure: OnFailure,
    },
    Error {
        name: String,
        error: String,
        required: bool,
    },
}

#[derive(Debug, Clone)]
pub struct GuardrailsOutcome {
    pub results: Vec<GuardResult>,
    pub blocked: bool,
    pub blocking_guard: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum GuardrailError {
    Unavailable(String),
    HttpError { status: u16, body: String },
    Timeout(String),
    ParseError(String),
}

impl std::fmt::Display for GuardrailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardrailError::Unavailable(msg) => write!(f, "Evaluator unavailable: {msg}"),
            GuardrailError::HttpError { status, body } => {
                write!(f, "HTTP error {status}: {body}")
            }
            GuardrailError::Timeout(msg) => write!(f, "Timeout: {msg}"),
            GuardrailError::ParseError(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl std::error::Error for GuardrailError {}
