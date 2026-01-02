## v0.7.2-dev

### Ci

- add security audit step
- enable dependency caching
- use dtolnay/rust-toolchain for stable environment

### Fix

- **providers**: migrate openai to structured logging

## v0.7.1 (2025-08-10)

### Fix

- fix bump package (#67)
- revert tower http request (#66)

## v0.7.0 (2025-08-10)

### Feat

- add reasoning support (#63)

### Fix

- make effort optional for gemini (#65)

## v0.6.2 (2025-08-07)

### Fix

- gemini enum support (#62)

## v0.6.1 (2025-08-06)

### Fix

- **deps**: revert commitizen tower version bump (#61)

## v0.6.0 (2025-08-06)

### Feat

- add gemini structure output (#60)

## v0.5.1 (2025-08-06)

### Fix

- gemini system prompt (#59)

## v0.5.0 (2025-08-03)

### Feat

- **models**: add filtered model info retrieval and response structures (#51)
- management API (#39)

### Fix

- **config**: allow env vars (#58)
- **Dockerfile**: specify compatible sqlx-cli version for edition2021 (#54)
- simplify string formatting to remove clippy warnings (#53)

## v0.4.5 (2025-06-25)

### Fix

- **deps**: revert commitizen unwanted chrono version change (#49)

## v0.4.4 (2025-06-24)

### Fix

- **bedrock**: handle ARN and inference profile identifiers without transformation (#48)
- **bedrock**: support IAM role auth (#47)

## v0.4.3 (2025-05-29)

### Fix

- make general optional again (#43)

## v0.4.2 (2025-05-22)

### Fix

- **tracing**: support disabling tracing of prompts and completions (#42)

## v0.4.1 (2025-05-20)

### Fix

- **openai**: support custom base URL (#40)
- **azure**: add support for custom base URL in AzureProvider endpoint (#41)

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
