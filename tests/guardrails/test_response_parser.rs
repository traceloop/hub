use hub_lib::guardrails::response_parser::*;
use hub_lib::guardrails::types::GuardrailError;

// ---------------------------------------------------------------------------
// Phase 3: Response Parser (8 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_successful_pass_response() {
    let body = r#"{"result": {"score": 0.95, "label": "safe"}, "pass": true}"#;
    let response = parse_evaluator_response(body).unwrap();
    assert!(response.pass);
    assert_eq!(response.result["score"], 0.95);
}

#[test]
fn test_parse_failed_response() {
    let body = r#"{"result": {"score": 0.2, "reason": "Toxic content"}, "pass": false}"#;
    let response = parse_evaluator_response(body).unwrap();
    assert!(!response.pass);
    assert_eq!(response.result["reason"], "Toxic content");
}

#[test]
fn test_parse_with_result_details() {
    let body = r#"{"result": {"score": 0.75, "label": "borderline", "categories": ["violence", "profanity"]}, "pass": true}"#;
    let response = parse_evaluator_response(body).unwrap();
    assert!(response.pass);
    assert_eq!(response.result["label"], "borderline");
    assert_eq!(response.result["categories"][0], "violence");
}

#[test]
fn test_parse_missing_pass_field() {
    let body = r#"{"result": {"score": 0.5}}"#;
    let result = parse_evaluator_response(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_malformed_json() {
    let body = "not json {at all";
    let result = parse_evaluator_response(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_non_json_response() {
    let body = "<html>Internal Server Error</html>";
    let result = parse_evaluator_response(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_empty_response_body() {
    let body = "";
    let result = parse_evaluator_response(body);
    assert!(result.is_err());
}

#[test]
fn test_parse_http_error_status() {
    let result = parse_evaluator_http_response(500, "Internal Server Error");
    assert!(result.is_err());
    match result.unwrap_err() {
        GuardrailError::HttpError { status, .. } => assert_eq!(status, 500),
        other => panic!("Expected HttpError, got {other:?}"),
    }
}
