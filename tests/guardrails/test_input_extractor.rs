use hub_lib::guardrails::input_extractor::*;
use hub_lib::models::content::{ChatCompletionMessage, ChatMessageContent, ChatMessageContentPart};

use super::helpers::*;

// ---------------------------------------------------------------------------
// Phase 2: Input Extractor (5 tests)
// ---------------------------------------------------------------------------

#[test]
fn test_extract_text_single_user_message() {
    let request = create_test_chat_request("Hello world");
    let text = extract_pre_call_input(&request);
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
    let text = extract_pre_call_input(&request);
    assert_eq!(text, "Follow-up question");
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
    let text = extract_pre_call_input(&request);
    assert_eq!(text, "Part 1 Part 2");
}

#[test]
fn test_extract_response_from_chat_completion() {
    let completion = create_test_chat_completion("Here is my response");
    let text = extract_post_call_input_from_completion(&completion);
    assert_eq!(text, "Here is my response");
}

#[test]
fn test_extract_handles_empty_content() {
    let mut request = create_test_chat_request("");
    request.messages[0].content = None;
    let text = extract_pre_call_input(&request);
    assert_eq!(text, "");
}
