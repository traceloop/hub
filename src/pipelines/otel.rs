use crate::models::chat::{ChatCompletion, ChatCompletionChoice, ChatCompletionRequest};
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::content::{ChatCompletionMessage, ChatMessageContent};
use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse};
use crate::models::streaming::ChatCompletionChunk;
use crate::models::usage::Usage;
use opentelemetry::global::{BoxedSpan, ObjectSafeSpan};
use opentelemetry::trace::{SpanKind, Status, Tracer};
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_semantic_conventions::attribute::GEN_AI_REQUEST_MODEL;
use opentelemetry_semantic_conventions::trace::*;
use std::collections::HashMap;

pub trait RecordSpan {
    fn record_span(&self, span: &mut BoxedSpan);
}

pub struct OtelTracer {
    span: BoxedSpan,
    accumulated_completion: Option<ChatCompletion>,
}

impl OtelTracer {
    pub fn init(endpoint: String, api_key: String) {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));

        let exporter: SpanExporter = SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_headers(headers)
            .build()
            .expect("Failed to initialize OpenTelemetry");

        let provider = TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .build();

        global::set_tracer_provider(provider);
    }

    pub fn start<T: RecordSpan>(operation: &str, request: &T) -> Self {
        let tracer = global::tracer("traceloop_hub");
        let mut span = tracer
            .span_builder(format!("traceloop_hub.{}", operation))
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
}

impl RecordSpan for ChatCompletionRequest {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new("llm.request.type", "chat"));
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

        for (i, message) in self.messages.iter().enumerate() {
            if let Some(content) = &message.content {
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.prompt.{}.role", i),
                    message.role.clone(),
                ));
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.prompt.{}.content", i),
                    match &content {
                        ChatMessageContent::String(content) => content.clone(),
                        ChatMessageContent::Array(content) => {
                            serde_json::to_string(content).unwrap_or_default()
                        }
                    },
                ));
            }
        }
    }
}

impl RecordSpan for ChatCompletion {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, self.model.clone()));
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_ID, self.id.clone()));

        self.usage.record_span(span);

        for choice in &self.choices {
            if let Some(content) = &choice.message.content {
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.role", choice.index),
                    choice.message.role.clone(),
                ));
                span.set_attribute(KeyValue::new(
                    format!("gen_ai.completion.{}.content", choice.index),
                    match &content {
                        ChatMessageContent::String(content) => content.clone(),
                        ChatMessageContent::Array(content) => {
                            serde_json::to_string(content).unwrap_or_default()
                        }
                    },
                ));
            }
            span.set_attribute(KeyValue::new(
                format!("gen_ai.completion.{}.finish_reason", choice.index),
                choice.finish_reason.clone().unwrap_or_default(),
            ));
        }
    }
}

impl RecordSpan for CompletionRequest {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new("llm.request.type", "completion"));
        span.set_attribute(KeyValue::new(GEN_AI_REQUEST_MODEL, self.model.clone()));
        span.set_attribute(KeyValue::new("gen_ai.prompt", self.prompt.clone()));

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
    }
}

impl RecordSpan for CompletionResponse {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, self.model.clone()));
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_ID, self.id.clone()));

        self.usage.record_span(span);

        for choice in &self.choices {
            span.set_attribute(KeyValue::new(
                format!("gen_ai.completion.{}.role", choice.index),
                "assistant".to_string(),
            ));
            span.set_attribute(KeyValue::new(
                format!("gen_ai.completion.{}.content", choice.index),
                choice.text.clone(),
            ));
            span.set_attribute(KeyValue::new(
                format!("gen_ai.completion.{}.finish_reason", choice.index),
                choice.finish_reason.clone().unwrap_or_default(),
            ));
        }
    }
}

impl RecordSpan for EmbeddingsRequest {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new("llm.request.type", "embeddings"));
        span.set_attribute(KeyValue::new(GEN_AI_REQUEST_MODEL, self.model.clone()));

        match &self.input {
            EmbeddingsInput::Single(text) => {
                span.set_attribute(KeyValue::new("llm.prompt.0.content", text.clone()));
            }
            EmbeddingsInput::Multiple(texts) => {
                for (i, text) in texts.iter().enumerate() {
                    span.set_attribute(KeyValue::new(
                        format!("llm.prompt.{}.role", i),
                        "user".to_string(),
                    ));
                    span.set_attribute(KeyValue::new(
                        format!("llm.prompt.{}.content", i),
                        text.clone(),
                    ));
                }
            }
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
            "gen_ai.usage.prompt_tokens",
            self.prompt_tokens as i64,
        ));
        span.set_attribute(KeyValue::new(
            "gen_ai.usage.completion_tokens",
            self.completion_tokens as i64,
        ));
        span.set_attribute(KeyValue::new(
            "gen_ai.usage.total_tokens",
            self.total_tokens as i64,
        ));
    }
}
