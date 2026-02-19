# Guardrails

Guardrails intercept LLM gateway traffic to evaluate requests and responses against configurable safety, validation, and quality checks. Each guard calls an external evaluator API and either **blocks** the request (HTTP 403) or **warns** via a response header, depending on configuration.

> **Available in:** Traceloop Hub v1
> **Full reference documentation:** [Guardrails Documentation](https://docs.traceloop.com/evaluators/guardrails)

---

Guardrails can be implemented in **Config Mode (Hub v1)** :
Guardrails fully defined in YAML configuration, applied automatically to gateway requests


This document focuses on **config mode** available in Traceloop Hub v1.

---

## How It Works

```
                 ┌──────────────┐
  Request ──────►│  Pre-call    │──── Block (403) ──► Client
                 │  Guards      │
                 └──────┬───────┘
                        │ pass
                        ▼
                 ┌──────────────┐
                 │  LLM Call    │
                 └──────┬───────┘
                        │
                        ▼
                 ┌──────────────┐
                 │  Post-call   │──── Block (403) ──► Client
                 │  Guards      │
                 └──────┬───────┘
                        │ pass
                        ▼
                   Response (+ warning headers if any)
```

1. **Pre-call guards** run on the user's prompt *before* it reaches the LLM.
2. **Post-call guards** run on the LLM's response *before* it is returned to the client.
3. All guards in a phase execute **concurrently** for minimal latency.

**Supported Routes:**
- ✅ Chat Completions (`/v1/chat/completions`) — pre-call and post-call guards
- ✅ Completions (`/v1/completions`) — pre-call and post-call guards
- ✅ Embeddings (`/v1/embeddings`) — **pre-call guards only**
- ⚠️ Streaming requests (`"stream": true`) — **pre-call guards only** (post-call guards are skipped because the response is sent as incremental chunks)

---

## Configuration

Guards are defined in the gateway YAML config under `guardrails`. Provider-level defaults for `api_base` and `api_key` can be inherited by guards or overridden per-guard.

```yaml
guardrails:
  providers:
    - name: traceloop
      api_base: https://api.traceloop.com
      api_key: ${TRACELOOP_API_KEY}

  guards:
    - name: pii-check
      provider: traceloop
      evaluator_slug: pii-detector
      mode: pre_call          # pre_call | post_call
      on_failure: block        # block | warn
      required: false          # when true, evaluator errors block the request; when false, they warn and continue (default: false)
      params:                  # evaluator-specific parameters
        probability_threshold: 0.7
```

Pipelines reference guards by name:

```yaml
pipelines:
  - name: default
    guards: [pii-check, injection-check]
    plugins:
      - model-router:
          models: [gpt-4]
```

### Runtime Guard Addition

Guards can be added (never removed) at request time via:
- **Header:** `X-Traceloop-Guardrails: extra-guard-1, extra-guard-2`

These are **additive** to the pipeline-configured guards, preserving the security baseline.

---

## Supported Evaluators

| Slug | Category | Configurable Params |
|---|---|---|
| `pii-detector` | Safety | `probability_threshold` |
| `secrets-detector` | Safety | — |
| `prompt-injection` | Safety | `threshold` |
| `profanity-detector` | Safety | — |
| `sexism-detector` | Safety | `threshold` |
| `toxicity-detector` | Safety | `threshold` |
| `regex-validator` | Validation | `regex`, `should_match`, `case_sensitive`, `dot_include_nl`, `multi_line` |
| `json-validator` | Validation | `enable_schema_validation`, `schema_string` |
| `sql-validator` | Validation | — |
| `tone-detection` | Quality | — |
| `prompt-perplexity` | Quality | — |
| `uncertainty-detector` | Quality | — |

---

## Failure Behavior

| Evaluation Result | `on_failure` | `required` | Action |
|---|---|---|---|
| Pass | — | — | Continue |
| Fail | `block` | — | Return 403 |
| Fail | `warn` | — | Add warning header, continue |
| Evaluator error | — | `true` | Return 403 (fail-closed) |
| Evaluator error | — | `false` | Add warning header, continue (fail-open) |

**Blocked response (403):**
```json
{
  "error": {
    "type": "guardrail_blocked",
    "guardrail": "pii-check",
    "message": "Request blocked by guardrail 'pii-check'",
    "evaluation_result": { ... },
    "reason": "evaluation_failed"
  }
}
```

**Warning header:**
```
X-Traceloop-Guardrail-Warning: guardrail_name="toxicity-filter", reason="failed"
```

---

## Observability

Each guard evaluation emits an OpenTelemetry child span with these attributes:

| Attribute | Description |
|---|---|
| `gen_ai.guardrail.name` | Guard name |
| `gen_ai.guardrail.status` | `PASSED`, `FAILED`, or `ERROR` |
| `gen_ai.guardrail.duration` | Evaluation time in ms |
| `gen_ai.guardrail.input` | Input text (when `trace_content_enabled`) |
| `gen_ai.guardrail.error.type` | Error category (`Unavailable`, `HttpError`, `Timeout`, `ParseError`) |
| `gen_ai.guardrail.error.message` | Error details |

---

## Implementation

See `src/guardrails/mod.rs` for module structure and key type definitions.
