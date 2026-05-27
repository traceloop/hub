# OTel `gen_ai.*messages` / `finish_reasons` test coverage

## Background

Branch `dk/otel-semconv-migration` migrates the OTel span emission in
`src/pipelines/otel.rs` to the GenAI semconv:

- `gen_ai.input.messages` and `gen_ai.output.messages` are emitted as
  serialized JSON arrays (replacing the previous per-message `gen_ai.prompt.N.*`
  and `gen_ai.completion.N.*` attributes).
- `gen_ai.response.finish_reasons` is emitted as a top-level string array,
  with values normalized through `map_finish_reason` (provider-specific values
  like `tool_calls`, `function_call`, `tool_use`, `end_turn`, `stop_sequence`,
  `max_tokens` map to the spec's `tool_call` / `stop` / `length`).
- Provider names use the well-known `gen_ai.provider.name` values
  (`openai`, `anthropic`, `azure.ai.openai`, `aws.bedrock`, `gcp.vertex_ai`).

The current `#[cfg(test)] mod tests` block covers the happy paths for
`map_finish_reason`, `top_level_finish_reasons`, and the most common
`build_input_messages` / `build_output_messages` shapes. It does not cover
empty inputs, multi-message conversations, the `name` field, simultaneous
content + tool_calls, multi-choice output, the `CompletionRequest` /
`CompletionResponse` JSON shapes, or the streaming accumulator.

## Goal

Add unit and streaming-integration tests so that the message-serialization
and finish-reason behavior is comprehensively covered, including the
streaming path that builds a `ChatCompletion` from a sequence of
`ChatCompletionChunk`s.

Out of scope: capturing real span attributes via an in-memory exporter
(introduces global tracer-provider state). Tests assert on the JSON the
helpers produce — which is exactly what gets set on the span.

## Design

### Refactor (test seams)

`CompletionRequest::record_span` and `CompletionResponse::record_span`
currently build their JSON inline. Extract two free helpers next to
`build_input_messages` / `build_output_messages`:

- `fn build_completion_input_messages(prompt: &str) -> String`
- `fn build_completion_output_messages(choices: &[CompletionChoice]) -> String`

Each `record_span` impl calls the helper inside `if get_trace_content_enabled()`.
Pure mechanical extraction — no behavior change.

### Helper unit tests

All added to the existing `#[cfg(test)] mod tests` in `src/pipelines/otel.rs`.

**`top_level_finish_reasons`**

- `[Some("stop")]` → `Some(vec!["stop"])`.
- All reasons mapped 1:1 when none are empty: `[Some("stop"), Some("max_tokens"), Some("tool_calls")]`
  → `Some(vec!["stop", "length", "tool_call"])`.

**`build_input_messages`**

- Empty slice → `"[]"`.
- Realistic conversation `system → user → assistant(content + tool_calls) → tool`:
  ordering preserved; roles emitted; assistant message's parts contain text
  before tool_call; tool message uses `tool_call_response`.
- Message with `name` field → JSON object includes `"name"`.
- Assistant message with both content and tool_calls → text parts precede
  tool_call parts.
- Multiple tool_calls in one message → each is its own part with its own
  id / name / arguments.
- Tool role with `Array` content → all `text` part `.text` values joined into
  the `response` string.
- Tool role with no `tool_call_id` → falls back to the generic path; no
  `tool_call_response` part is emitted; content surfaces as text parts.

**`build_output_messages`**

- Empty choices → `"[]"`.
- Multi-choice with mixed finish reasons (`stop`, `max_tokens`, `tool_calls`,
  `None`) → JSON array length matches; each `finish_reason` mapped correctly.
- Choice with `name` field → JSON object includes `"name"`.
- Choice with content + tool_calls → both kinds of parts present in order.

**`build_completion_input_messages`**

- Non-empty prompt → single user message with a single text part containing
  the prompt.
- Empty prompt → still a well-formed single-message array with an empty
  `content` string.

**`build_completion_output_messages`**

- Empty choices → `"[]"`.
- Multi-choice with `max_tokens` / `stop` / `None` → finish reasons mapped
  to `length` / `stop` / `""`; each choice carries one `text` part with
  `c.text` as its `content`.

### Streaming-integration tests

These drive `OtelTracer::log_chunk` and assert on the private
`accumulated_completion` field (the `tests` mod has access to private
fields — no extra accessor needed). Each test then feeds
`accumulated_completion.unwrap()` through `build_output_messages` and
`top_level_finish_reasons` to verify the JSON the real `streaming_end()`
would emit.

A small fixture helper builds a `ChatCompletionChunk` with a single
`Choice` from `(index, role, content, tool_calls, finish_reason)`.

Cases:

1. **Single chunk** with `role=assistant`, `content="Hi"`, `finish_reason="stop"`
   → `choices[0].message.content == Some(String("Hi"))`,
   `choices[0].finish_reason == Some("stop")`.
   `build_output_messages` yields one message with `finish_reason: "stop"`.

2. **Multi-chunk content concat**: chunk1 sets `role=assistant`,
   `content="Hello "`; chunk2 `content="world"`; chunk3
   `finish_reason="tool_calls"` only → concatenated content `"Hello world"`;
   final `finish_reason` mapped to `"tool_call"`.

3. **Tool-call streaming**: chunk1 `role=assistant`; chunk2 delivers
   `tool_calls=[ChatMessageToolCall { id: "c1", name: "f", arguments: "{}" }]`;
   chunk3 `finish_reason="tool_use"` → accumulated choice has `tool_calls`
   set; `build_output_messages` includes a `tool_call` part and
   `finish_reason: "tool_call"`.

4. **Multi-choice interleaved**: chunks alternate `index: 0` and `index: 1`;
   choice 0 closes with `finish_reason="end_turn"`, choice 1 with `"max_tokens"`
   → both accumulate independently; `top_level_finish_reasons` over them
   returns `Some(vec!["stop", "length"])`.

5. **`streaming_end` no-op**: calling `streaming_end()` on a fresh
   `OtelTracer` with no chunks logged does not panic and leaves
   `accumulated_completion == None`.

## Files touched

- `src/pipelines/otel.rs`
  - Add `build_completion_input_messages` and `build_completion_output_messages`.
  - Call them from `RecordSpan for CompletionRequest` and
    `RecordSpan for CompletionResponse` (replacing the inline `json!` blocks).
  - Expand `#[cfg(test)] mod tests` with the cases listed above.

No other files change.

## Risks / non-goals

- Tests do not assert on real `KeyValue` attributes set on a `BoxedSpan`.
  The helpers' return values *are* the strings that get set, so this is
  equivalent in practice and avoids global tracer-provider state.
- Tests do not exercise `OtelTracer::init` / exporter setup — unchanged
  by this work.
