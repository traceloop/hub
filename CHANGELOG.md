## v0.4.0 (2025-05-16)

### Feat

- **provider**: add Google VertexAI support (#24)
- support AWS bedrock base models (#25)
- add max_completion_tokens to ChatCompletionRequest (#36)
- support structured output (#33)

### Fix

- replace eprintln with tracing info for API request errors in Azure and OpenAI providers (#37)
- make optional json_schema field to ResponseFormat (#35)

## v0.3.0 (2025-03-04)

### Feat

- add logprobs and top_logprobs options to ChatCompletionRequest (#27)

### Fix

- **cd**: correct docker hub secrets (#31)
- **azure**: embeddings structs improvement (#29)
- add proper error logging for azure and openai calls (#18)
- **anthropic**: separate system from messages (#17)

## v0.2.1 (2024-12-01)

### Fix

- tool call support (#16)
- restructure providers, separate request/response conversion (#15)

## v0.2.0 (2024-11-25)

### Feat

- **openai**: support streaming (#10)
- add prometheus metrics (#13)
- **cd**: deploy to traceloop on workflow distpatch (#11)

### Fix

- config file path from env var instead of command argument (#12)

## v0.1.0 (2024-11-16)

### Feat

- otel support (#7)
- implement pipeline steering logic (#5)
- dynamic pipeline routing (#4)
- azure openai provider (#3)
- initial completions and embeddings routes with openai and anthropic providers (#1)

### Fix

- dockerfile and release pipeline (#2)
- make anthropic work (#8)
- cleanups; lint warnings fail CI (#9)
- missing model name in response; 404 for model not found (#6)
