use crate::config::lib::get_trace_content_enabled;
use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::completion::{CompletionChoice, CompletionRequest, CompletionResponse};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{EmbeddingsRequest, EmbeddingsResponse};
use crate::models::streaming::ChatCompletionChunk;
use crate::models::tool_calls::ChatMessageToolCall;
use crate::models::usage::{EmbeddingUsage, Usage};
use opentelemetry::global::{BoxedSpan, ObjectSafeSpan};
use opentelemetry::trace::{SpanKind, Status, Tracer};
use opentelemetry::{Array, KeyValue, StringValue, Value as OtelValue, global};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_semantic_conventions::trace::{
    GEN_AI_OPERATION_NAME, GEN_AI_REQUEST_ENCODING_FORMATS, GEN_AI_REQUEST_FREQUENCY_PENALTY,
    GEN_AI_REQUEST_MAX_TOKENS, GEN_AI_REQUEST_MODEL, GEN_AI_REQUEST_PRESENCE_PENALTY,
    GEN_AI_REQUEST_TEMPERATURE, GEN_AI_REQUEST_TOP_P, GEN_AI_RESPONSE_FINISH_REASONS,
    GEN_AI_RESPONSE_ID, GEN_AI_RESPONSE_MODEL, GEN_AI_USAGE_INPUT_TOKENS,
    GEN_AI_USAGE_OUTPUT_TOKENS,
};
use serde_json::{Value, json};
use std::collections::HashMap;

// Attributes not exported by opentelemetry-semantic-conventions v0.31:
// - `gen_ai.provider.name`, `gen_ai.input.messages`, `gen_ai.output.messages` are
//   Development-stage spec additions still absent from the Rust crate.
// - `gen_ai.usage.total_tokens` is defined by the spec but the Rust crate removed
//   its constant in v0.31. Swap these out when upstream catches up.
const GEN_AI_PROVIDER_NAME: &str = "gen_ai.provider.name";
const GEN_AI_INPUT_MESSAGES: &str = "gen_ai.input.messages";
const GEN_AI_OUTPUT_MESSAGES: &str = "gen_ai.output.messages";
const GEN_AI_USAGE_TOTAL_TOKENS: &str = "gen_ai.usage.total_tokens";

fn string_array(values: Vec<String>) -> OtelValue {
    let v: Vec<StringValue> = values.into_iter().map(StringValue::from).collect();
    OtelValue::Array(Array::String(v))
}

pub trait RecordSpan {
    fn record_span(&self, span: &mut BoxedSpan);
}

pub struct OtelTracer {
    span: BoxedSpan,
    accumulated_completion: Option<ChatCompletion>,
}

impl OtelTracer {
    pub fn init(endpoint: String, api_key: String) {
        // Clone endpoint for use in error messages
        let endpoint_for_error = endpoint.clone();

        // Try to get the current runtime handle
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // Spawn the initialization task on the runtime
            handle.spawn(async move {
                // Use spawn_blocking for the potentially blocking OpenTelemetry initialization
                let result = tokio::task::spawn_blocking(move || {
                    global::set_text_map_propagator(TraceContextPropagator::new());
                    let mut headers = HashMap::new();
                    headers.insert("Authorization".to_string(), format!("Bearer {api_key}"));

                    let exporter_result = SpanExporter::builder()
                        .with_http()
                        .with_endpoint(endpoint.clone())
                        .with_headers(headers)
                        .build();

                    let exporter = match exporter_result {
                        Ok(exporter) => exporter,
                        Err(e) => {
                            tracing::error!("Failed to initialize OpenTelemetry exporter for endpoint {}: {}. Tracing will be disabled.", endpoint, e);
                            return Err(e);
                        }
                    };

                    let provider = TracerProvider::builder()
                        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
                        .build();

                    global::set_tracer_provider(provider);
                    tracing::debug!("OpenTelemetry tracer initialized successfully for endpoint: {}", endpoint);
                    Ok(())
                }).await;

                match result {
                    Ok(Ok(())) => {
                        // Successfully initialized
                    }
                    Ok(Err(e)) => {
                        tracing::error!("OpenTelemetry initialization failed: {}. Tracing will be disabled.", e);
                    }
                    Err(e) => {
                        tracing::error!("OpenTelemetry initialization task failed: {}. Tracing will be disabled.", e);
                    }
                }
            });

            // Log that initialization was started asynchronously
            tracing::debug!(
                "OpenTelemetry initialization started asynchronously for endpoint: {}",
                endpoint_for_error
            );
        } else {
            tracing::error!(
                "No Tokio runtime available for OpenTelemetry initialization. Tracing will be disabled."
            );
        }
    }

    pub fn start<T: RecordSpan>(operation: &str, request: &T) -> Self {
        let tracer = global::tracer("traceloop_hub");
        let mut span = tracer
            .span_builder(format!("traceloop_hub.{operation}"))
            .with_kind(SpanKind::Client)
            .start(&tracer);

        request.record_span(&mut span);

        Self {
            span,
            accumulated_completion: None,
        }
    }

    pub fn log_chunk(&mut self, chunk: &ChatCompletionChunk) {
        if self.accumulated_completion.is_none() {
            self.accumulated_completion = Some(ChatCompletion {
                id: chunk.id.clone(),
                object: None,
                created: None,
                model: chunk.model.clone(),
                choices: vec![],
                usage: Usage::default(),
                system_fingerprint: chunk.system_fingerprint.clone(),
            });
        }

        if let Some(completion) = &mut self.accumulated_completion {
            for chunk_choice in &chunk.choices {
                if let Some(existing_choice) =
                    completion.choices.get_mut(chunk_choice.index as usize)
                {
                    if let Some(content) = &chunk_choice.delta.content {
                        if let Some(ChatMessageContent::String(existing_content)) =
                            &mut existing_choice.message.content
                        {
                            existing_content.push_str(content);
                        }
                    }
                    if chunk_choice.finish_reason.is_some() {
                        existing_choice.finish_reason = chunk_choice.finish_reason.clone();
                    }
                    if let Some(tool_calls) = &chunk_choice.delta.tool_calls {
                        existing_choice.message.tool_calls = Some(tool_calls.clone());
                    }
                } else {
                    completion.choices.push(ChatCompletionChoice {
                        index: chunk_choice.index,
                        message: ChatCompletionMessage {
                            name: None,
                            role: chunk_choice
                                .delta
                                .role
                                .clone()
                                .unwrap_or_else(|| "assistant".to_string()),
                            content: Some(ChatMessageContent::String(
                                chunk_choice.delta.content.clone().unwrap_or_default(),
                            )),
                            tool_calls: chunk_choice.delta.tool_calls.clone(),
                            tool_call_id: None,
                            refusal: None,
                        },
                        finish_reason: chunk_choice.finish_reason.clone(),
                        logprobs: None,
                    });
                }
            }
        }
    }

    pub fn streaming_end(&mut self) {
        if let Some(completion) = self.accumulated_completion.take() {
            completion.record_span(&mut self.span);
            self.span.set_status(Status::Ok);
        }
    }

    pub fn log_success<T: RecordSpan>(&mut self, response: &T) {
        response.record_span(&mut self.span);
        self.span.set_status(Status::Ok);
    }

    pub fn log_error(&mut self, description: String) {
        self.span.set_status(Status::error(description));
    }

    pub fn set_vendor(&mut self, vendor: &str) {
        self.span
            .set_attribute(KeyValue::new(GEN_AI_PROVIDER_NAME, vendor.to_string()));
    }
}

fn map_finish_reason(reason: Option<&str>) -> String {
    match reason {
        None => String::new(),
        Some(raw) => match raw {
            "" => String::new(),
            "tool_calls" | "function_call" | "tool_use" => "tool_call".to_string(),
            "end_turn" | "stop_sequence" => "stop".to_string(),
            "max_tokens" => "length".to_string(),
            other => other.to_string(),
        },
    }
}

fn top_level_finish_reasons<'a, I>(reasons: I) -> Option<Vec<String>>
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    let mapped: Vec<String> = reasons
        .into_iter()
        .map(map_finish_reason)
        .filter(|s| !s.is_empty())
        .collect();
    if mapped.is_empty() { None } else { Some(mapped) }
}

fn parse_tool_arguments(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
}

fn content_to_text_parts(content: &ChatMessageContent) -> Vec<Value> {
    match content {
        ChatMessageContent::String(s) => vec![json!({ "type": "text", "content": s })],
        ChatMessageContent::Array(parts) => parts
            .iter()
            .map(|p| json!({ "type": p.r#type, "content": p.text }))
            .collect(),
    }
}

fn tool_call_parts(calls: &[ChatMessageToolCall]) -> Vec<Value> {
    calls
        .iter()
        .map(|c| {
            json!({
                "type": "tool_call",
                "id": c.id,
                "name": c.function.name,
                "arguments": parse_tool_arguments(&c.function.arguments),
            })
        })
        .collect()
}

fn input_message_json(message: &ChatCompletionMessage) -> Value {
    // Tool-role messages carry a tool_call_response part keyed by tool_call_id.
    if message.role == "tool" {
        if let Some(id) = &message.tool_call_id {
            let response_text = match &message.content {
                Some(ChatMessageContent::String(s)) => s.clone(),
                Some(ChatMessageContent::Array(parts)) => parts
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join(""),
                None => String::new(),
            };
            let mut obj = json!({
                "role": "tool",
                "parts": [ { "type": "tool_call_response", "id": id, "response": response_text } ],
            });
            if let Some(name) = &message.name {
                obj["name"] = Value::String(name.clone());
            }
            return obj;
        }
    }

    let mut parts = match &message.content {
        Some(content) => content_to_text_parts(content),
        None => Vec::new(),
    };
    if let Some(calls) = &message.tool_calls {
        parts.extend(tool_call_parts(calls));
    }

    let mut obj = json!({ "role": message.role, "parts": parts });
    if let Some(name) = &message.name {
        obj["name"] = Value::String(name.clone());
    }
    obj
}

fn build_input_messages(messages: &[ChatCompletionMessage]) -> String {
    let arr: Vec<Value> = messages.iter().map(input_message_json).collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

fn output_message_json(choice: &ChatCompletionChoice) -> Value {
    let mut parts = match &choice.message.content {
        Some(content) => content_to_text_parts(content),
        None => Vec::new(),
    };
    if let Some(calls) = &choice.message.tool_calls {
        parts.extend(tool_call_parts(calls));
    }
    let mut obj = json!({
        "role": choice.message.role,
        "parts": parts,
        "finish_reason": map_finish_reason(choice.finish_reason.as_deref()),
    });
    if let Some(name) = &choice.message.name {
        obj["name"] = Value::String(name.clone());
    }
    obj
}

fn build_output_messages(choices: &[ChatCompletionChoice]) -> String {
    let arr: Vec<Value> = choices.iter().map(output_message_json).collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

fn build_completion_input_messages(prompt: &str) -> String {
    let messages = json!([
        { "role": "user", "parts": [ { "type": "text", "content": prompt } ] }
    ]);
    messages.to_string()
}

fn build_completion_output_messages(choices: &[CompletionChoice]) -> String {
    let arr: Vec<Value> = choices
        .iter()
        .map(|c| {
            json!({
                "role": "assistant",
                "parts": [ { "type": "text", "content": c.text } ],
                "finish_reason": map_finish_reason(c.finish_reason.as_deref()),
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

fn set_finish_reasons<'a, I>(span: &mut BoxedSpan, reasons: I)
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    if let Some(values) = top_level_finish_reasons(reasons) {
        span.set_attribute(KeyValue::new(
            GEN_AI_RESPONSE_FINISH_REASONS,
            string_array(values),
        ));
    }
}

impl RecordSpan for ChatCompletionRequest {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_OPERATION_NAME, "chat"));
        span.set_attribute(KeyValue::new(GEN_AI_REQUEST_MODEL, self.model.clone()));

        if let Some(freq_penalty) = self.frequency_penalty {
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_FREQUENCY_PENALTY,
                freq_penalty as f64,
            ));
        }
        if let Some(pres_penalty) = self.presence_penalty {
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_PRESENCE_PENALTY,
                pres_penalty as f64,
            ));
        }
        if let Some(top_p) = self.top_p {
            span.set_attribute(KeyValue::new(GEN_AI_REQUEST_TOP_P, top_p as f64));
        }
        if let Some(temp) = self.temperature {
            span.set_attribute(KeyValue::new(GEN_AI_REQUEST_TEMPERATURE, temp as f64));
        }
        if let Some(max_tokens) = self.max_tokens {
            span.set_attribute(KeyValue::new(GEN_AI_REQUEST_MAX_TOKENS, max_tokens as i64));
        }

        if get_trace_content_enabled() {
            span.set_attribute(KeyValue::new(
                GEN_AI_INPUT_MESSAGES,
                build_input_messages(&self.messages),
            ));
        }
    }
}

impl RecordSpan for ChatCompletion {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, self.model.clone()));
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_ID, self.id.clone()));

        self.usage.record_span(span);

        set_finish_reasons(
            span,
            self.choices.iter().map(|c| c.finish_reason.as_deref()),
        );

        if get_trace_content_enabled() {
            span.set_attribute(KeyValue::new(
                GEN_AI_OUTPUT_MESSAGES,
                build_output_messages(&self.choices),
            ));
        }
    }
}

impl RecordSpan for CompletionRequest {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_OPERATION_NAME, "text_completion"));
        span.set_attribute(KeyValue::new(GEN_AI_REQUEST_MODEL, self.model.clone()));

        if let Some(freq_penalty) = self.frequency_penalty {
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_FREQUENCY_PENALTY,
                freq_penalty as f64,
            ));
        }
        if let Some(pres_penalty) = self.presence_penalty {
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_PRESENCE_PENALTY,
                pres_penalty as f64,
            ));
        }
        if let Some(top_p) = self.top_p {
            span.set_attribute(KeyValue::new(GEN_AI_REQUEST_TOP_P, top_p as f64));
        }
        if let Some(temp) = self.temperature {
            span.set_attribute(KeyValue::new(GEN_AI_REQUEST_TEMPERATURE, temp as f64));
        }
        if let Some(max_tokens) = self.max_tokens {
            span.set_attribute(KeyValue::new(GEN_AI_REQUEST_MAX_TOKENS, max_tokens as i64));
        }

        if get_trace_content_enabled() {
            span.set_attribute(KeyValue::new(
                GEN_AI_INPUT_MESSAGES,
                build_completion_input_messages(&self.prompt),
            ));
        }
    }
}

impl RecordSpan for CompletionResponse {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, self.model.clone()));
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_ID, self.id.clone()));

        self.usage.record_span(span);

        set_finish_reasons(
            span,
            self.choices.iter().map(|c| c.finish_reason.as_deref()),
        );

        if get_trace_content_enabled() {
            span.set_attribute(KeyValue::new(
                GEN_AI_OUTPUT_MESSAGES,
                build_completion_output_messages(&self.choices),
            ));
        }
    }
}

impl RecordSpan for EmbeddingsRequest {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_OPERATION_NAME, "embeddings"));
        span.set_attribute(KeyValue::new(GEN_AI_REQUEST_MODEL, self.model.clone()));

        if let Some(encoding_format) = &self.encoding_format {
            span.set_attribute(KeyValue::new(
                GEN_AI_REQUEST_ENCODING_FORMATS,
                string_array(vec![encoding_format.clone()]),
            ));
        }
    }
}

impl RecordSpan for EmbeddingsResponse {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, self.model.clone()));

        self.usage.record_span(span);
    }
}

impl RecordSpan for Usage {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(
            GEN_AI_USAGE_INPUT_TOKENS,
            self.prompt_tokens as i64,
        ));
        span.set_attribute(KeyValue::new(
            GEN_AI_USAGE_OUTPUT_TOKENS,
            self.completion_tokens as i64,
        ));
        span.set_attribute(KeyValue::new(
            GEN_AI_USAGE_TOTAL_TOKENS,
            self.total_tokens as i64,
        ));
    }
}

impl RecordSpan for EmbeddingUsage {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(
            GEN_AI_USAGE_INPUT_TOKENS,
            self.prompt_tokens.unwrap_or(0) as i64,
        ));
        span.set_attribute(KeyValue::new(
            GEN_AI_USAGE_TOTAL_TOKENS,
            self.total_tokens.unwrap_or(0) as i64,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::content::ChatMessageContentPart;
    use crate::models::tool_calls::FunctionCall;
    use crate::providers::provider::get_vendor_name;
    use crate::types::ProviderType;

    #[test]
    fn test_get_vendor_name_mappings() {
        assert_eq!(get_vendor_name(&ProviderType::OpenAI), "openai");
        assert_eq!(get_vendor_name(&ProviderType::Azure), "azure.ai.openai");
        assert_eq!(get_vendor_name(&ProviderType::Anthropic), "anthropic");
        assert_eq!(get_vendor_name(&ProviderType::Bedrock), "aws.bedrock");
        assert_eq!(get_vendor_name(&ProviderType::VertexAI), "gcp.vertex_ai");
    }

    #[test]
    fn test_set_vendor_method_exists() {
        let mut tracer = OtelTracer {
            span: opentelemetry::global::tracer("test").start("test"),
            accumulated_completion: None,
        };

        tracer.set_vendor("openai");
        tracer.set_vendor("anthropic");
        tracer.set_vendor("azure.ai.openai");
    }

    #[test]
    fn test_map_finish_reason_provider_variants() {
        assert_eq!(map_finish_reason(None), "");
        assert_eq!(map_finish_reason(Some("")), "");
        assert_eq!(map_finish_reason(Some("stop")), "stop");
        assert_eq!(map_finish_reason(Some("length")), "length");
        assert_eq!(map_finish_reason(Some("content_filter")), "content_filter");
        assert_eq!(map_finish_reason(Some("tool_calls")), "tool_call");
        assert_eq!(map_finish_reason(Some("function_call")), "tool_call");
        assert_eq!(map_finish_reason(Some("tool_use")), "tool_call");
        assert_eq!(map_finish_reason(Some("end_turn")), "stop");
        assert_eq!(map_finish_reason(Some("stop_sequence")), "stop");
        assert_eq!(map_finish_reason(Some("max_tokens")), "length");
        assert_eq!(map_finish_reason(Some("custom_reason")), "custom_reason");
    }

    #[test]
    fn test_top_level_finish_reasons_omits_when_all_missing() {
        let none: Vec<Option<&str>> = vec![None, Some(""), None];
        assert!(top_level_finish_reasons(none).is_none());
    }

    #[test]
    fn test_top_level_finish_reasons_keeps_order_and_maps() {
        let reasons = vec![Some("stop"), None, Some("tool_calls"), Some("end_turn")];
        let out = top_level_finish_reasons(reasons).unwrap();
        assert_eq!(out, vec!["stop", "tool_call", "stop"]);
    }

    fn user_text_message(text: &str) -> ChatCompletionMessage {
        ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::String(text.to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        }
    }

    #[test]
    fn test_build_input_messages_string_content() {
        let msgs = vec![user_text_message("hi")];
        let json_str = build_input_messages(&msgs);
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v[0]["role"], "user");
        assert_eq!(v[0]["parts"][0]["type"], "text");
        assert_eq!(v[0]["parts"][0]["content"], "hi");
    }

    #[test]
    fn test_build_input_messages_array_content_preserves_parts() {
        let msg = ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(ChatMessageContent::Array(vec![
                ChatMessageContentPart {
                    r#type: "text".to_string(),
                    text: "first".to_string(),
                },
                ChatMessageContentPart {
                    r#type: "text".to_string(),
                    text: "second".to_string(),
                },
            ])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        };
        let json_str = build_input_messages(&[msg]);
        let v: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v[0]["parts"].as_array().unwrap().len(), 2);
        assert_eq!(v[0]["parts"][0]["content"], "first");
        assert_eq!(v[0]["parts"][1]["content"], "second");
    }

    #[test]
    fn test_build_input_messages_assistant_with_tool_call_parses_arguments() {
        let msg = ChatCompletionMessage {
            role: "assistant".to_string(),
            content: None,
            name: None,
            tool_calls: Some(vec![ChatMessageToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"city":"NYC"}"#.to_string(),
                },
            }]),
            tool_call_id: None,
            refusal: None,
        };
        let v: Value = serde_json::from_str(&build_input_messages(&[msg])).unwrap();
        assert_eq!(v[0]["role"], "assistant");
        let part = &v[0]["parts"][0];
        assert_eq!(part["type"], "tool_call");
        assert_eq!(part["id"], "call_1");
        assert_eq!(part["name"], "get_weather");
        assert_eq!(part["arguments"]["city"], "NYC");
    }

    #[test]
    fn test_build_input_messages_tool_call_arguments_fallback_to_string() {
        let msg = ChatCompletionMessage {
            role: "assistant".to_string(),
            content: None,
            name: None,
            tool_calls: Some(vec![ChatMessageToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "noop".to_string(),
                    arguments: "not json".to_string(),
                },
            }]),
            tool_call_id: None,
            refusal: None,
        };
        let v: Value = serde_json::from_str(&build_input_messages(&[msg])).unwrap();
        assert_eq!(v[0]["parts"][0]["arguments"], "not json");
    }

    #[test]
    fn test_build_input_messages_tool_role_becomes_tool_call_response() {
        let msg = ChatCompletionMessage {
            role: "tool".to_string(),
            content: Some(ChatMessageContent::String("72F sunny".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
            refusal: None,
        };
        let v: Value = serde_json::from_str(&build_input_messages(&[msg])).unwrap();
        assert_eq!(v[0]["role"], "tool");
        assert_eq!(v[0]["parts"][0]["type"], "tool_call_response");
        assert_eq!(v[0]["parts"][0]["id"], "call_1");
        assert_eq!(v[0]["parts"][0]["response"], "72F sunny");
    }

    #[test]
    fn test_build_output_messages_finish_reason_required_even_when_absent() {
        let choice = ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: "assistant".to_string(),
                content: Some(ChatMessageContent::String("hello".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            },
            finish_reason: None,
            logprobs: None,
        };
        let v: Value = serde_json::from_str(&build_output_messages(&[choice])).unwrap();
        assert_eq!(v[0]["finish_reason"], "");
    }

    #[test]
    fn test_build_output_messages_maps_finish_reason() {
        let choice = ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_calls: Some(vec![ChatMessageToolCall {
                    id: "c1".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "f".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
                tool_call_id: None,
                refusal: None,
            },
            finish_reason: Some("tool_calls".to_string()),
            logprobs: None,
        };
        let v: Value = serde_json::from_str(&build_output_messages(&[choice])).unwrap();
        assert_eq!(v[0]["finish_reason"], "tool_call");
        assert_eq!(v[0]["parts"][0]["type"], "tool_call");
    }

    fn msg(
        role: &str,
        content: Option<ChatMessageContent>,
        name: Option<&str>,
        tool_calls: Option<Vec<ChatMessageToolCall>>,
        tool_call_id: Option<&str>,
    ) -> ChatCompletionMessage {
        ChatCompletionMessage {
            role: role.to_string(),
            content,
            name: name.map(|s| s.to_string()),
            tool_calls,
            tool_call_id: tool_call_id.map(|s| s.to_string()),
            refusal: None,
        }
    }

    #[test]
    fn test_top_level_finish_reasons_single() {
        let out = top_level_finish_reasons(vec![Some("stop")]).unwrap();
        assert_eq!(out, vec!["stop"]);
    }

    #[test]
    fn test_top_level_finish_reasons_all_mapped() {
        let out = top_level_finish_reasons(vec![
            Some("stop"),
            Some("max_tokens"),
            Some("tool_calls"),
        ])
        .unwrap();
        assert_eq!(out, vec!["stop", "length", "tool_call"]);
    }

    #[test]
    fn test_build_input_messages_empty_slice() {
        assert_eq!(build_input_messages(&[]), "[]");
    }

    #[test]
    fn test_build_input_messages_multi_turn_conversation() {
        let messages = vec![
            msg(
                "system",
                Some(ChatMessageContent::String("be brief".to_string())),
                None,
                None,
                None,
            ),
            msg(
                "user",
                Some(ChatMessageContent::String("weather?".to_string())),
                None,
                None,
                None,
            ),
            msg(
                "assistant",
                Some(ChatMessageContent::String("Looking it up".to_string())),
                None,
                Some(vec![ChatMessageToolCall {
                    id: "call_1".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"city":"NYC"}"#.to_string(),
                    },
                }]),
                None,
            ),
            msg(
                "tool",
                Some(ChatMessageContent::String("72F".to_string())),
                None,
                None,
                Some("call_1"),
            ),
        ];
        let v: Value = serde_json::from_str(&build_input_messages(&messages)).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0]["role"], "system");
        assert_eq!(arr[1]["role"], "user");
        assert_eq!(arr[2]["role"], "assistant");
        assert_eq!(arr[3]["role"], "tool");

        let assistant_parts = arr[2]["parts"].as_array().unwrap();
        assert_eq!(assistant_parts.len(), 2);
        assert_eq!(assistant_parts[0]["type"], "text");
        assert_eq!(assistant_parts[0]["content"], "Looking it up");
        assert_eq!(assistant_parts[1]["type"], "tool_call");

        assert_eq!(arr[3]["parts"][0]["type"], "tool_call_response");
    }

    #[test]
    fn test_build_input_messages_name_field() {
        let m = msg(
            "user",
            Some(ChatMessageContent::String("hi".to_string())),
            Some("alice"),
            None,
            None,
        );
        let v: Value = serde_json::from_str(&build_input_messages(&[m])).unwrap();
        assert_eq!(v[0]["name"], "alice");
    }

    #[test]
    fn test_build_input_messages_assistant_content_and_tool_calls() {
        let m = msg(
            "assistant",
            Some(ChatMessageContent::String("calling".to_string())),
            None,
            Some(vec![ChatMessageToolCall {
                id: "c1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "f".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
            None,
        );
        let v: Value = serde_json::from_str(&build_input_messages(&[m])).unwrap();
        let parts = v[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "tool_call");
    }

    #[test]
    fn test_build_input_messages_multiple_tool_calls() {
        let m = msg(
            "assistant",
            None,
            None,
            Some(vec![
                ChatMessageToolCall {
                    id: "c1".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "a".to_string(),
                        arguments: "{}".to_string(),
                    },
                },
                ChatMessageToolCall {
                    id: "c2".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "b".to_string(),
                        arguments: "{}".to_string(),
                    },
                },
            ]),
            None,
        );
        let v: Value = serde_json::from_str(&build_input_messages(&[m])).unwrap();
        let parts = v[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["id"], "c1");
        assert_eq!(parts[0]["name"], "a");
        assert_eq!(parts[1]["id"], "c2");
        assert_eq!(parts[1]["name"], "b");
    }

    #[test]
    fn test_build_input_messages_tool_role_array_content_joined() {
        let m = msg(
            "tool",
            Some(ChatMessageContent::Array(vec![
                ChatMessageContentPart {
                    r#type: "text".to_string(),
                    text: "a".to_string(),
                },
                ChatMessageContentPart {
                    r#type: "text".to_string(),
                    text: "b".to_string(),
                },
            ])),
            None,
            None,
            Some("call_1"),
        );
        let v: Value = serde_json::from_str(&build_input_messages(&[m])).unwrap();
        assert_eq!(v[0]["parts"][0]["type"], "tool_call_response");
        assert_eq!(v[0]["parts"][0]["response"], "ab");
    }

    #[test]
    fn test_build_input_messages_tool_role_without_id_fallback() {
        let m = msg(
            "tool",
            Some(ChatMessageContent::String("result".to_string())),
            None,
            None,
            None,
        );
        let v: Value = serde_json::from_str(&build_input_messages(&[m])).unwrap();
        let parts = v[0]["parts"].as_array().unwrap();
        for part in parts {
            assert_ne!(part["type"], "tool_call_response");
        }
        assert_eq!(parts[0]["type"], "text");
    }

    #[test]
    fn test_build_output_messages_empty_choices() {
        assert_eq!(build_output_messages(&[]), "[]");
    }

    #[test]
    fn test_build_output_messages_multi_choice_mixed_reasons() {
        let make = |reason: Option<&str>| ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: "assistant".to_string(),
                content: Some(ChatMessageContent::String("x".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            },
            finish_reason: reason.map(|s| s.to_string()),
            logprobs: None,
        };
        let choices = vec![
            make(Some("stop")),
            make(Some("max_tokens")),
            make(Some("tool_calls")),
            make(None),
        ];
        let v: Value = serde_json::from_str(&build_output_messages(&choices)).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0]["finish_reason"], "stop");
        assert_eq!(arr[1]["finish_reason"], "length");
        assert_eq!(arr[2]["finish_reason"], "tool_call");
        assert_eq!(arr[3]["finish_reason"], "");
    }

    #[test]
    fn test_build_output_messages_name_field() {
        let choice = ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: "assistant".to_string(),
                content: Some(ChatMessageContent::String("hi".to_string())),
                name: Some("bob".to_string()),
                tool_calls: None,
                tool_call_id: None,
                refusal: None,
            },
            finish_reason: Some("stop".to_string()),
            logprobs: None,
        };
        let v: Value = serde_json::from_str(&build_output_messages(&[choice])).unwrap();
        assert_eq!(v[0]["name"], "bob");
    }

    #[test]
    fn test_build_output_messages_content_and_tool_calls() {
        let choice = ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: "assistant".to_string(),
                content: Some(ChatMessageContent::String("calling".to_string())),
                name: None,
                tool_calls: Some(vec![ChatMessageToolCall {
                    id: "c1".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "f".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
                tool_call_id: None,
                refusal: None,
            },
            finish_reason: Some("tool_calls".to_string()),
            logprobs: None,
        };
        let v: Value = serde_json::from_str(&build_output_messages(&[choice])).unwrap();
        let parts = v[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "tool_call");
    }

    #[test]
    fn test_build_completion_input_messages_basic() {
        let v: Value =
            serde_json::from_str(&build_completion_input_messages("hello")).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["role"], "user");
        let parts = arr[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["content"], "hello");
    }

    #[test]
    fn test_build_completion_input_messages_empty_prompt() {
        let v: Value =
            serde_json::from_str(&build_completion_input_messages("")).unwrap();
        assert_eq!(v[0]["parts"][0]["content"], "");
    }

    #[test]
    fn test_build_completion_output_messages_empty() {
        assert_eq!(build_completion_output_messages(&[]), "[]");
    }

    #[test]
    fn test_build_completion_output_messages_multi_choice_mapping() {
        let choices = vec![
            CompletionChoice {
                text: "first".to_string(),
                index: 0,
                logprobs: None,
                finish_reason: Some("max_tokens".to_string()),
            },
            CompletionChoice {
                text: "second".to_string(),
                index: 1,
                logprobs: None,
                finish_reason: Some("stop".to_string()),
            },
            CompletionChoice {
                text: "third".to_string(),
                index: 2,
                logprobs: None,
                finish_reason: None,
            },
        ];
        let v: Value =
            serde_json::from_str(&build_completion_output_messages(&choices)).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["finish_reason"], "length");
        assert_eq!(arr[0]["parts"][0]["content"], "first");
        assert_eq!(arr[1]["finish_reason"], "stop");
        assert_eq!(arr[1]["parts"][0]["content"], "second");
        assert_eq!(arr[2]["finish_reason"], "");
        assert_eq!(arr[2]["parts"][0]["content"], "third");
    }

    use crate::models::streaming::{Choice, ChoiceDelta};

    fn chunk(
        index: u32,
        role: Option<&str>,
        content: Option<&str>,
        tool_calls: Option<Vec<ChatMessageToolCall>>,
        finish_reason: Option<&str>,
    ) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "chunk_id".to_string(),
            model: "test".to_string(),
            created: 0,
            service_tier: None,
            system_fingerprint: None,
            usage: None,
            choices: vec![Choice {
                index,
                delta: ChoiceDelta {
                    role: role.map(|s| s.to_string()),
                    content: content.map(|s| s.to_string()),
                    tool_calls,
                    reasoning: None,
                },
                finish_reason: finish_reason.map(|s| s.to_string()),
                logprobs: None,
            }],
        }
    }

    fn fresh_tracer() -> OtelTracer {
        OtelTracer {
            span: opentelemetry::global::tracer("test").start("test"),
            accumulated_completion: None,
        }
    }

    #[test]
    fn test_streaming_single_chunk_with_finish_reason() {
        let mut tracer = fresh_tracer();
        tracer.log_chunk(&chunk(0, Some("assistant"), Some("Hi"), None, Some("stop")));

        let completion = tracer.accumulated_completion.as_ref().unwrap();
        assert_eq!(completion.choices.len(), 1);
        let choice = &completion.choices[0];
        match &choice.message.content {
            Some(ChatMessageContent::String(s)) => assert_eq!(s, "Hi"),
            _ => panic!("expected string content"),
        }
        assert_eq!(choice.finish_reason.as_deref(), Some("stop"));

        let v: Value =
            serde_json::from_str(&build_output_messages(&completion.choices)).unwrap();
        assert_eq!(v[0]["finish_reason"], "stop");
        assert_eq!(v[0]["parts"][0]["type"], "text");
        assert_eq!(v[0]["parts"][0]["content"], "Hi");
    }

    #[test]
    fn test_streaming_multi_chunk_content_concat() {
        let mut tracer = fresh_tracer();
        tracer.log_chunk(&chunk(0, Some("assistant"), Some("Hello "), None, None));
        tracer.log_chunk(&chunk(0, None, Some("world"), None, None));
        tracer.log_chunk(&chunk(0, None, None, None, Some("tool_calls")));

        let completion = tracer.accumulated_completion.as_ref().unwrap();
        let choice = &completion.choices[0];
        match &choice.message.content {
            Some(ChatMessageContent::String(s)) => assert_eq!(s, "Hello world"),
            _ => panic!("expected string content"),
        }

        let v: Value =
            serde_json::from_str(&build_output_messages(&completion.choices)).unwrap();
        assert_eq!(v[0]["finish_reason"], "tool_call");
    }

    #[test]
    fn test_streaming_tool_call_chunk() {
        let mut tracer = fresh_tracer();
        tracer.log_chunk(&chunk(0, Some("assistant"), None, None, None));
        tracer.log_chunk(&chunk(
            0,
            None,
            None,
            Some(vec![ChatMessageToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"city":"NYC"}"#.to_string(),
                },
            }]),
            None,
        ));
        tracer.log_chunk(&chunk(0, None, None, None, Some("tool_use")));

        let completion = tracer.accumulated_completion.as_ref().unwrap();
        let choice = &completion.choices[0];
        assert!(choice.message.tool_calls.is_some());

        let v: Value =
            serde_json::from_str(&build_output_messages(&completion.choices)).unwrap();
        assert_eq!(v[0]["finish_reason"], "tool_call");
        let parts = v[0]["parts"].as_array().unwrap();
        assert!(parts.iter().any(|p| p["type"] == "tool_call"));
    }

    #[test]
    fn test_streaming_multi_choice_interleaved() {
        let mut tracer = fresh_tracer();
        tracer.log_chunk(&chunk(0, Some("assistant"), Some("a-"), None, None));
        tracer.log_chunk(&chunk(1, Some("assistant"), Some("b-"), None, None));
        tracer.log_chunk(&chunk(0, None, Some("first"), None, None));
        tracer.log_chunk(&chunk(1, None, Some("second"), None, None));
        tracer.log_chunk(&chunk(0, None, None, None, Some("end_turn")));
        tracer.log_chunk(&chunk(1, None, None, None, Some("max_tokens")));

        let completion = tracer.accumulated_completion.as_ref().unwrap();
        assert_eq!(completion.choices.len(), 2);
        match &completion.choices[0].message.content {
            Some(ChatMessageContent::String(s)) => assert_eq!(s, "a-first"),
            _ => panic!("expected string content"),
        }
        match &completion.choices[1].message.content {
            Some(ChatMessageContent::String(s)) => assert_eq!(s, "b-second"),
            _ => panic!("expected string content"),
        }

        let out = top_level_finish_reasons(
            completion
                .choices
                .iter()
                .map(|c| c.finish_reason.as_deref()),
        )
        .unwrap();
        assert_eq!(out, vec!["stop", "length"]);
    }

    #[test]
    fn test_streaming_end_is_safe_with_no_chunks() {
        let mut tracer = fresh_tracer();
        tracer.streaming_end();
        assert!(tracer.accumulated_completion.is_none());
    }
}
