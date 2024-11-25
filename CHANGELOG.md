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
