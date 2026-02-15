use hub_lib::guardrails::evaluator_types::get_evaluator;
use hub_lib::guardrails::providers::traceloop::TraceloopClient;
use hub_lib::guardrails::types::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use wiremock::matchers;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::helpers::*;

// ---------------------------------------------------------------------------
// Infrastructure
// ---------------------------------------------------------------------------

struct EvaluatorTestCase {
    slug: &'static str,
    cassette_name: &'static str,
    input: &'static str,
    params: HashMap<String, Value>,
    expected_pass: bool,
}

#[derive(Deserialize)]
struct Cassette {
    response_body: Value,
}

fn load_cassette(name: &str) -> Cassette {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cassettes/guardrails")
        .join(format!("{}.json", name));
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Cassette '{}' not found at {:?}.", name, path));
    serde_json::from_str(&content).expect("Failed to parse cassette JSON")
}

/// Set up a wiremock server, execute the guard, and verify the request was correct.
async fn run_evaluator_test(tc: &EvaluatorTestCase) {
    let cassette = load_cassette(tc.cassette_name);

    // Build the expected request body using the evaluator's build_body()
    let evaluator = get_evaluator(tc.slug).unwrap();
    let expected_body = evaluator.build_body(tc.input, &tc.params).unwrap();

    // Set up wiremock with strict matchers that verify the request
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path(format!(
            "/v2/guardrails/execute/{}",
            tc.slug
        )))
        .and(matchers::header("Authorization", "Bearer test-api-key"))
        .and(matchers::header("Content-Type", "application/json"))
        .and(matchers::body_json(&expected_body))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(&cassette.response_body),
        )
        .expect(1)
        .mount(&server)
        .await;

    // Create guard pointing at the mock server and execute
    let mut guard =
        create_test_guard_with_api_base(tc.slug, GuardMode::PreCall, &server.uri());
    guard.evaluator_slug = tc.slug.to_string();
    guard.params = tc.params.clone();

    let client = TraceloopClient::new();
    let result = client.evaluate(&guard, tc.input).await.unwrap();

    // Verify the response was interpreted correctly
    assert_eq!(
        result.pass, tc.expected_pass,
        "{}: expected pass={}, got pass={}",
        tc.slug, tc.expected_pass, result.pass
    );
    // wiremock .expect(1) verifies the request matched all matchers
}

// ---------------------------------------------------------------------------
// 1. PII Detector (text body, optional probability_threshold config)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cassette_pii_detector() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "pii-detector",
        cassette_name: "pii_detector_pass",
        input: "The weather is sunny today and I like programming in Rust.",
        params: HashMap::new(),
        expected_pass: true,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 2. Secrets Detector (text body, no config)
// ---------------------------------------------------------------------------

#[ignore = "secrets-detector returns HTTP 500 on current API"]
#[tokio::test]
async fn test_cassette_secrets_detector() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "secrets-detector",
        cassette_name: "secrets_detector_pass",
        input: "Here is a simple function that adds two numbers together.",
        params: HashMap::new(),
        expected_pass: true,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 3. Prompt Injection (prompt body, optional threshold config)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cassette_prompt_injection() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "prompt-injection",
        cassette_name: "prompt_injection_pass",
        input: "What is the capital of France?",
        params: HashMap::new(),
        expected_pass: true,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 4. Profanity Detector (text body, no config)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cassette_profanity_detector() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "profanity-detector",
        cassette_name: "profanity_detector_fail",
        input: "This is damn bullshit and I think it's a total crap product.",
        params: HashMap::new(),
        expected_pass: false,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 5. Sexism Detector (text body, threshold config)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cassette_sexism_detector() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "sexism-detector",
        cassette_name: "sexism_detector_fail",
        input: "Women should not be in leadership positions because they are too emotional.",
        params: HashMap::from([("threshold".to_string(), json!(0.5))]),
        expected_pass: false,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 6. Toxicity Detector (text body, threshold config)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cassette_toxicity_detector() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "toxicity-detector",
        cassette_name: "toxicity_detector_fail",
        input: "You are a complete idiot and everyone hates you. You should be ashamed.",
        params: HashMap::from([("threshold".to_string(), json!(0.5))]),
        expected_pass: false,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 7. Regex Validator (text body, regex config)
// ---------------------------------------------------------------------------

#[ignore = "regex-validator returns HTTP 500 on current API"]
#[tokio::test]
async fn test_cassette_regex_validator() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "regex-validator",
        cassette_name: "regex_validator_pass",
        input: "Order ID: ABC-12345",
        params: HashMap::from([
            ("regex".to_string(), json!(r"[A-Z]{3}-\d{5}")),
            ("should_match".to_string(), json!(true)),
        ]),
        expected_pass: true,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 8. JSON Validator (text body, optional schema config)
// ---------------------------------------------------------------------------

#[ignore = "json-validator returns HTTP 500 on current API"]
#[tokio::test]
async fn test_cassette_json_validator() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "json-validator",
        cassette_name: "json_validator_pass",
        input: r#"{"name": "Alice", "age": 30}"#,
        params: HashMap::new(),
        expected_pass: true,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 9. SQL Validator (text body, no config)
// ---------------------------------------------------------------------------

#[ignore = "sql-validator returns HTTP 500 on current API"]
#[tokio::test]
async fn test_cassette_sql_validator() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "sql-validator",
        cassette_name: "sql_validator_pass",
        input: "SELECT id, name FROM users WHERE active = true ORDER BY name",
        params: HashMap::new(),
        expected_pass: true,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 10. Tone Detection (text body, no config)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cassette_tone_detection() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "tone-detection",
        cassette_name: "tone_detection_fail",
        input: "This is ABSOLUTELY UNACCEPTABLE. I DEMAND to speak to someone competent immediately!",
        params: HashMap::new(),
        expected_pass: false,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 11. Prompt Perplexity (prompt body, no config)
// ---------------------------------------------------------------------------

#[ignore = "prompt-perplexity returns HTTP 500 on current API"]
#[tokio::test]
async fn test_cassette_prompt_perplexity() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "prompt-perplexity",
        cassette_name: "prompt_perplexity_pass",
        input: "Please explain the concept of photosynthesis in simple terms.",
        params: HashMap::new(),
        expected_pass: true,
    })
    .await;
}

// ---------------------------------------------------------------------------
// 12. Uncertainty Detector (prompt body, no config)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cassette_uncertainty_detector() {
    run_evaluator_test(&EvaluatorTestCase {
        slug: "uncertainty-detector",
        cassette_name: "uncertainty_detector_pass",
        input: "What is 2 + 2?",
        params: HashMap::new(),
        expected_pass: true,
    })
    .await;
}
