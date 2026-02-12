use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use thiserror::Error;

use super::providers::GuardrailClient;

/// Shared guardrail resources: resolved guards + client.
/// Built once per router build and shared across all pipelines.
pub type GuardrailResources = (Arc<Vec<Guard>>, Arc<dyn GuardrailClient>);

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
pub struct Guard {
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

impl Eq for Guard {}

impl Hash for Guard {
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
    #[serde(
        default,
        deserialize_with = "deserialize_providers",
        serialize_with = "serialize_providers"
    )]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub guards: Vec<Guard>,
}

fn deserialize_providers<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, ProviderConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    let list: Vec<ProviderConfig> = Vec::deserialize(deserializer)?;
    Ok(list.into_iter().map(|p| (p.name.clone(), p)).collect())
}

fn serialize_providers<S>(
    providers: &HashMap<String, ProviderConfig>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let list: Vec<&ProviderConfig> = providers.values().collect();
    list.serialize(serializer)
}

impl Hash for GuardrailsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut entries: Vec<_> = self.providers.iter().collect();
        entries.sort_by_key(|(k, _)| (*k).clone());
        for (k, v) in entries {
            k.hash(state);
            v.hash(state);
        }
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
pub struct GuardWarning {
    pub guard_name: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct GuardrailsOutcome {
    pub results: Vec<GuardResult>,
    pub blocked: bool,
    pub blocking_guard: Option<String>,
    pub warnings: Vec<GuardWarning>,
}

#[derive(Debug, Clone, Error)]
pub enum GuardrailError {
    #[error("Evaluator unavailable: {0}")]
    Unavailable(String),

    #[error("HTTP error {status}: {body}")]
    HttpError { status: u16, body: String },

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<reqwest::Error> for GuardrailError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            GuardrailError::Timeout(e.to_string())
        } else {
            GuardrailError::Unavailable(e.to_string())
        }
    }
}

/// Guardrails state attached to a pipeline, containing resolved guards and client.
///
/// `all_guards` and `client` are shared across all pipelines via `Arc` (built once).
/// `pipeline_guard_names` holds the guard names declared by this specific pipeline.
/// At request time, guards are resolved by merging pipeline guards with any
/// additional guards specified via the `X-Traceloop-Guardrails` header.
#[derive(Clone)]
pub struct Guardrails {
    pub all_guards: Arc<Vec<Guard>>,
    pub pipeline_guard_names: Vec<String>,
    pub client: Arc<dyn GuardrailClient>,
}
