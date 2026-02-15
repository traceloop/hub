use hub_lib::guardrails::parsing::{
    CompletionExtractor, PromptExtractor, parse_evaluator_http_response, parse_evaluator_response,
};
use hub_lib::guardrails::types::GuardrailError;
use hub_lib::models::content::{ChatCompletionMessage, ChatMessageContent, ChatMessageContentPart};

use super::helpers::*;

// ---------------------------------------------------------------------------
// Input Extraction (5 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_extract_text_single_user_message() {
    let request = create_test_chat_request("Hello world");
    let text = request.extract_pompt();
    assert_eq!(text, "Hello world");
}

#[test]
fn test_extract_text_multi_turn_conversation() {
    let mut request = default_request();
    request.messages = vec![
        ChatCompletionMessage {
            role: "system".to_string(),
            content: Some(ChatMessageContent::String("You are helpful".to_string())),
            ..default_message()
        },
        ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("First question".to_string())),
            ..default_message()
        },
        ChatCompletionMessage {
            role: "assistant".to_string(),
            content: Some(ChatMessageContent::String("First answer".to_string())),
            ..default_message()
        },
        ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String("Follow-up question".to_string())),
            ..default_message()
        },
    ];
    let text = request.extract_pompt();
    assert_eq!(
        text,
        "You are helpful\nFirst question\nFirst answer\nFollow-up question"
    );
}

#[test]
fn test_extract_text_from_array_content_parts() {
    let mut request = create_test_chat_request("");
    request.messages[0].content = Some(ChatMessageContent::Array(vec![
        ChatMessageContentPart {
            r#type: "text".to_string(),
            text: "Part 1".to_string(),
        },
        ChatMessageContentPart {
            r#type: "text".to_string(),
            text: "Part 2".to_string(),
        },
    ]));
    let text = request.extract_pompt();
    assert_eq!(text, "Part 1 Part 2");
}

#[test]
fn test_extract_response_from_chat_completion() {
    let completion = create_test_chat_completion("Here is my response");
    let text = completion.extract_completion();
    assert_eq!(text, "Here is my response");
}

#[test]
fn test_extract_handles_empty_content() {
    let mut request = create_test_chat_request("");
    request.messages[0].content = None;
    let text = request.extract_pompt();
    assert_eq!(text, "");
}

// ---------------------------------------------------------------------------
// Response Parsing (8 tests)
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
