use crate::models::chat::{ChatCompletionRequest, ChatCompletionResponse, ChatMessageContent};
use crate::models::common::Usage;
use crate::models::completion::{CompletionRequest, CompletionResponse};
use crate::models::embeddings::{EmbeddingsInput, EmbeddingsRequest, EmbeddingsResponse};
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

        Self { span }
    }

    pub fn log_success<T: RecordSpan>(&mut self, response: &T) {
        response.record_span(&mut self.span);
        self.span.set_status(Status::Ok);
    }

    pub fn log_error(&mut self) {
        self.span.set_status(Status::error("Not Found"));
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
            span.set_attribute(KeyValue::new(
                format!("gen_ai.prompt.{}.role", i),
                message.role.clone(),
            ));
            span.set_attribute(KeyValue::new(
                format!("gen_ai.prompt.{}.content", i),
                match &message.content {
                    ChatMessageContent::String(content) => content.clone(),
                    ChatMessageContent::Array(content) => {
                        serde_json::to_string(content).unwrap_or_default()
                    }
                },
            ));
        }
    }
}

impl RecordSpan for ChatCompletionResponse {
    fn record_span(&self, span: &mut BoxedSpan) {
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_MODEL, self.model.clone()));
        span.set_attribute(KeyValue::new(GEN_AI_RESPONSE_ID, self.id.clone()));

        self.usage.record_span(span);

        for choice in &self.choices {
            span.set_attribute(KeyValue::new(
                format!("gen_ai.completion.{}.role", choice.index),
                choice.message.role.clone(),
            ));
            span.set_attribute(KeyValue::new(
                format!("gen_ai.completion.{}.content", choice.index),
                match &choice.message.content {
                    ChatMessageContent::String(content) => content.clone(),
                    ChatMessageContent::Array(content) => {
                        serde_json::to_string(content).unwrap_or_default()
                    }
                },
            ));
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
