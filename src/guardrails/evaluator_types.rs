use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use std::collections::HashMap;

use super::types::GuardrailError;

// ---------------------------------------------------------------------------
// Slugs
// ---------------------------------------------------------------------------

// Safety
pub const PII_DETECTOR: &str = "pii-detector";
pub const SECRETS_DETECTOR: &str = "secrets-detector";
pub const PROMPT_INJECTION: &str = "prompt-injection";
pub const PROFANITY_DETECTOR: &str = "profanity-detector";
pub const SEXISM_DETECTOR: &str = "sexism-detector";
pub const TOXICITY_DETECTOR: &str = "toxicity-detector";
// Validators
pub const REGEX_VALIDATOR: &str = "regex-validator";
pub const JSON_VALIDATOR: &str = "json-validator";
pub const SQL_VALIDATOR: &str = "sql-validator";
// Quality and adherence
pub const TONE_DETECTION: &str = "tone-detection";
pub const PROMPT_PERPLEXITY: &str = "prompt-perplexity";
pub const UNCERTAINTY_DETECTOR: &str = "uncertainty-detector";

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Each supported evaluator implements this trait to build its typed request body.
pub trait EvaluatorRequest: Send + Sync {
    fn build_body(
        &self,
        input: &str,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, GuardrailError>;
}

/// Look up the evaluator implementation for a given slug.
pub fn get_evaluator(slug: &str) -> Option<&'static dyn EvaluatorRequest> {
    match slug {
        // Safety
        PII_DETECTOR => Some(&PiiDetector),
        SECRETS_DETECTOR => Some(&SecretsDetector),
        PROMPT_INJECTION => Some(&PromptInjection),
        PROFANITY_DETECTOR => Some(&ProfanityDetector),
        SEXISM_DETECTOR => Some(&SexismDetector),
        TOXICITY_DETECTOR => Some(&ToxicityDetector),
        // Validators
        REGEX_VALIDATOR => Some(&RegexValidator),
        JSON_VALIDATOR => Some(&JsonValidator),
        SQL_VALIDATOR => Some(&SqlValidator),
        // Quality and adherence
        TONE_DETECTION => Some(&ToneDetection),
        PROMPT_PERPLEXITY => Some(&PromptPerplexity),
        UNCERTAINTY_DETECTOR => Some(&UncertaintyDetector),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn text_body(input: &str) -> serde_json::Value {
    json!({ "input": { "text": input } })
}

fn prompt_body(input: &str) -> serde_json::Value {
    json!({ "input": { "prompt": input } })
}

/// Deserialize `params` into a typed config `C`, then attach it to the body.
/// Skips the `config` key entirely when `params` is empty.
fn attach_config<C: Default + DeserializeOwned + Serialize>(
    mut body: serde_json::Value,
    params: &HashMap<String, serde_json::Value>,
    slug: &str,
) -> Result<serde_json::Value, GuardrailError> {
    if params.is_empty() {
        return Ok(body);
    }
    let params_value: serde_json::Value = params.clone().into_iter().collect();
    let config: C = serde_json::from_value(params_value)
        .map_err(|e| GuardrailError::ParseError(format!("{slug}: invalid config — {e}")))?;
    let config_json =
        serde_json::to_value(config).map_err(|e| GuardrailError::ParseError(e.to_string()))?;
    if config_json.as_object().is_some_and(|m| !m.is_empty()) {
        body["config"] = config_json;
    }
    Ok(body)
}

macro_rules! evaluator_with_no_config {
    ($name:ident, $body_fn:ident) => {
        pub struct $name;
        impl EvaluatorRequest for $name {
            fn build_body(
                &self,
                input: &str,
                _params: &HashMap<String, serde_json::Value>,
            ) -> Result<serde_json::Value, GuardrailError> {
                Ok($body_fn(input))
            }
        }
    };
}

macro_rules! evaluator_with_config {
    ($name:ident, $body_fn:ident, $config:ty, $slug:expr) => {
        pub struct $name;
        impl EvaluatorRequest for $name {
            fn build_body(
                &self,
                input: &str,
                params: &HashMap<String, serde_json::Value>,
            ) -> Result<serde_json::Value, GuardrailError> {
                attach_config::<$config>($body_fn(input), params, $slug)
            }
        }
    };
}

evaluator_with_no_config!(SecretsDetector, text_body);
evaluator_with_no_config!(ProfanityDetector, text_body);
evaluator_with_no_config!(SqlValidator, text_body);
evaluator_with_no_config!(ToneDetection, text_body);
evaluator_with_no_config!(PromptPerplexity, prompt_body);
evaluator_with_no_config!(UncertaintyDetector, prompt_body);

// ---------------------------------------------------------------------------
// Config structs  (mirroring the Go DTOs in evaluator_mbt.go)
// ---------------------------------------------------------------------------

#[derive(Default, Deserialize, Serialize)]
pub struct PiiDetectorConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability_threshold: Option<f64>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct ThresholdConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct RegexValidatorConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub should_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_sensitive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dot_include_nl: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_line: Option<bool>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct JsonValidatorConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_schema_validation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_string: Option<String>,
}

// ---------------------------------------------------------------------------
// Evaluators with config
// ---------------------------------------------------------------------------

evaluator_with_config!(PiiDetector, text_body, PiiDetectorConfig, PII_DETECTOR);
evaluator_with_config!(
    PromptInjection,
    prompt_body,
    ThresholdConfig,
    PROMPT_INJECTION
);
evaluator_with_config!(SexismDetector, text_body, ThresholdConfig, SEXISM_DETECTOR);
evaluator_with_config!(
    ToxicityDetector,
    text_body,
    ThresholdConfig,
    TOXICITY_DETECTOR
);
evaluator_with_config!(
    RegexValidator,
    text_body,
    RegexValidatorConfig,
    REGEX_VALIDATOR
);
evaluator_with_config!(
    JsonValidator,
    text_body,
    JsonValidatorConfig,
    JSON_VALIDATOR
);
