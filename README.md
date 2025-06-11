# Hub

<p align="center">
<a href="https://www.traceloop.com/docs/hub#gh-light-mode-only">
<img width="300" src="https://raw.githubusercontent.com/traceloop/hub/main/img/logo-light.png">
</a>
<a href="https://www.traceloop.com/docs/hub#gh-dark-mode-only">
<img width="300" src="https://raw.githubusercontent.com/traceloop/hub/main/img/logo-dark.png">
</a>
</p>
<p align="center">
  <p align="center">Open-source, high-performance LLM gateway written in Rust. Connect to any LLM provider with a single API. Observability Included.</p>
</p>
<h4 align="center">
    <a href="https://traceloop.com/docs/hub/getting-started"><strong>Get started »</strong></a>
    <br />
    <br />
  <a href="https://traceloop.com/slack">Slack</a> |
  <a href="https://traceloop.com/docs/hub">Docs</a>
</h4>

<h4 align="center">
  <a href="https://github.com/traceloop/hub/releases">
    <img src="https://img.shields.io/github/release/traceloop/hub">
  </a>
   <a href="https://github.com/traceloop/hub/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-Apache 2.0-blue.svg" alt="Traceloop Hub is released under the Apache-2.0 License">
  </a>
  <a href="https://github.com/traceloop/hub/actions/workflows/ci.yml">
  <img src="https://github.com/traceloop/hub/actions/workflows/ci.yml/badge.svg">
  </a>
  <a href="https://github.com/traceloop/hub/issues">
    <img src="https://img.shields.io/github/commit-activity/m/traceloop/hub" alt="git commit activity" />
  </a>
  <a href="https://www.ycombinator.com/companies/traceloop"><img src="https://img.shields.io/website?color=%23f26522&down_message=Y%20Combinator&label=Backed&logo=ycombinator&style=flat-square&up_message=Y%20Combinator&url=https%3A%2F%2Fwww.ycombinator.com"></a>
  <a href="https://github.com/traceloop/hub/blob/main/CONTRIBUTING.md">
    <img src="https://img.shields.io/badge/PRs-Welcome-brightgreen" alt="PRs welcome!" />
  </a>
  <a href="https://traceloop.com/slack">
    <img src="https://img.shields.io/badge/chat-on%20Slack-blueviolet" alt="Slack community channel" />
  </a>
  <a href="https://twitter.com/traceloopdev">
    <img src="https://img.shields.io/badge/follow-%40traceloopdev-1DA1F2?logo=twitter&style=social" alt="Traceloop Twitter" />
  </a>
</h4>

Hub is a next generation smart proxy for LLM applications. It centralizes control and tracing of all LLM calls and traces.
It's built in Rust so it's fast and efficient. It's completely open-source and free to use.

Built and maintained by Traceloop under a dual-license model.

## 🚀 Getting Started

Make sure to copy a `config.yaml` file from `config-example.yaml` and set the correct values, following the [configuration](https://www.traceloop.com/docs/hub/configuration) instructions.

You can then run the hub using the docker image:

```
docker run --rm -p 3000:3000 -v $(pwd)/config.yaml:/etc/hub/config.yaml:ro -e CONFIG_FILE_PATH='/etc/hub/config.yaml'  -t traceloop/hub
```

You can also run it locally. Make sure you have `rust` v1.82 and above installed and then run:

```
cargo run
```

Connect to the hub by using the OpenAI SDK on any language, and setting the base URL to:

```
http://localhost:3000/api/v1
```

For example, in Python:

```
client = OpenAI(
    base_url="http://localhost:3000/api/v1",
    api_key=os.getenv("OPENAI_API_KEY"),
    # default_headers={"x-traceloop-pipeline": "azure-only"},
)
completion = client.chat.completions.create(
    model="claude-3-5-sonnet-20241022",
    messages=[{"role": "user", "content": "Tell me a joke about opentelemetry"}],
    max_tokens=1000,
)
```

## 🌱 Contributing

Whether big or small, we love contributions ❤️ Check out our guide to see how to [get started](https://traceloop.com/docs/hub/contributing/overview).

Not sure where to get started? You can:

- [Book a free pairing session with one of our teammates](mailto:nir@traceloop.com?subject=Pairing%20session&body=I'd%20like%20to%20do%20a%20pairing%20session!)!
- Join our <a href="https://traceloop.com/slack">Slack</a>, and ask us any questions there.

## 💚 Community & Support

- [Slack](https://traceloop.com/slack) (For live discussion with the community and the Traceloop team)
- [GitHub Discussions](https://github.com/traceloop/hub/discussions) (For help with building and deeper conversations about features)
- [GitHub Issues](https://github.com/traceloop/hub/issues) (For any bugs and errors you encounter using OpenLLMetry)
- [Twitter](https://twitter.com/traceloopdev) (Get news fast)

## Supported Providers

- OpenAI
- Anthropic
- Azure OpenAI
- Google VertexAI (Gemini)

## Configuration

See `config-example.yaml` for a complete configuration example.

### Provider Configuration

#### OpenAI

```yaml
providers:
  - key: openai
    type: openai
    api_key: "<your-openai-api-key>"
```

#### Azure OpenAI

```yaml
providers:
  - key: azure-openai
    type: azure
    api_key: "<your-azure-api-key>"
    resource_name: "<your-resource-name>"
    api_version: "<your-api-version>"
```

#### Google VertexAI (Gemini)

```yaml
providers:
  - key: vertexai
    type: vertexai
    api_key: "<your-gcp-api-key>"
    project_id: "<your-gcp-project-id>"
    location: "<your-gcp-region>"
    credentials_path: "/path/to/service-account.json"
```

Authentication Methods:
1. API Key Authentication:
   - Set the `api_key` field with your GCP API key
   - Leave `credentials_path` empty
2. Service Account Authentication:
   - Set `credentials_path` to your service account JSON file path
   - Can also use `GOOGLE_APPLICATION_CREDENTIALS` environment variable
   - Leave `api_key` empty when using service account auth

Supported Features:
- Chat Completions (with Gemini models)
- Text Completions
- Embeddings
- Streaming Support
- Function/Tool Calling
- Multi-modal Inputs (images + text)

Example Model Configuration:
```yaml
models:
  # Chat and Completion model
  - key: gemini-1.5-flash
    type: gemini-1.5-flash
    provider: vertexai
  
  # Embeddings model
  - key: textembedding-gecko
    type: textembedding-gecko
    provider: vertexai
```

Example Usage with OpenAI SDK:
```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:3000/api/v1",
    api_key="not-needed-for-vertexai"
)

# Chat completion
response = client.chat.completions.create(
    model="gemini-1.5-flash",
    messages=[{"role": "user", "content": "Tell me a joke"}]
)

# Embeddings
response = client.embeddings.create(
    model="textembedding-gecko",
    input="Sample text for embedding"
)
```

### Pipeline Configuration

```yaml
pipelines:
  - name: default
    type: chat
    plugins:
      - model-router:
          models:
            - gemini-pro
```

## Development

### Running Tests

The test suite uses recorded HTTP interactions (cassettes) to make tests reproducible without requiring actual API credentials.

To run tests:
```bash
cargo test
```

To record new test cassettes:
1. Set up your API credentials:
   - For service account auth: Set `VERTEXAI_CREDENTIALS_PATH` to your service account key file path
   - For API key auth: Use the test with API key (currently marked as ignored)
2. Delete the existing cassette files in `tests/cassettes/vertexai/`
3. Run the tests with recording enabled:
```bash
RECORD_MODE=1 cargo test
```

Additional test configurations:
- `RETRY_DELAY`: Set the delay in seconds between retries when hitting quota limits (default: 60)
- Tests automatically retry up to 3 times when hitting quota limits

Note: Some tests may be marked as `#[ignore]` if they require specific credentials or are not ready for general use.

## License

Traceloop Hub is a commercial open source company, which means some parts of this open source repository require a commercial license. The concept is called "Open Core" where the core technology is fully open source, licensed under Apache 2.0 and the enterprise features are covered under a commercial license (`/ee` Enterprise Edition).

### Our Philosophy

All core LLM gateway functionality is open-source under Apache 2.0. Enterprise features that provide additional value for larger organizations are under a commercial license.

| Apache 2.0 (Core) | Enterprise Edition |
| --- | --- |
| ✅ Self-host for commercial purposes | ✅ Self-host for commercial purposes |
| ✅ Clone privately | ✅ Clone privately |
| ✅ Fork publicly | ✅ Fork publicly |
| ✅ Modify and distribute | ✅ Modify and distribute |
| ✅ Core LLM Gateway | ✅ Core LLM Gateway |
| ✅ Provider Integrations | ✅ Provider Integrations |
| ✅ YAML Configuration | ✅ YAML Configuration |
| ❌ Management REST API | ✅ Management REST API |
| ❌ Database-driven Configuration | ✅ Database-driven Configuration |
| ❌ Dynamic Configuration Updates | ✅ Dynamic Configuration Updates |
| ❌ Zero-downtime Reloading | ✅ Zero-downtime Reloading |

### License Structure

- **Core Hub (`/src`, `/Cargo.toml`)**: Licensed under [Apache 2.0](LICENSE)
- **Enterprise Edition (`/ee`)**: Licensed under [Traceloop Enterprise License](ee/LICENSE.EE)

### Using the Enterprise Edition

The content of the `/ee` folder is copyrighted and you are not allowed to use this code to host your own version without obtaining a proper license first. However, open-sourcing the enterprise content brings transparency to our product suite and shows that there are no unknown caveats or backdoors in the commercial part of our business.

For enterprise licensing inquiries, please contact us at [enterprise@traceloop.com](mailto:enterprise@traceloop.com).

### Building Different Versions

- **Open Source Build**: `cargo build` (default, no enterprise features)
- **Enterprise Build**: `cargo build --features ee_feature` (includes enterprise features)

The enterprise features are conditionally compiled and only available when building with the `ee_feature` flag.

### Deployment Options

#### Helm Chart Deployment

The Hub LLM Gateway includes a Helm chart that supports both OSS and Enterprise Edition deployments:

- **OSS Deployment**: Uses static YAML configuration files
- **EE Deployment**: Uses PostgreSQL database with dynamic configuration management

For detailed EE deployment instructions, see [docs/EE_HELM_DEPLOYMENT.md](docs/EE_HELM_DEPLOYMENT.md).

Quick EE deployment:
```bash
# Create PostgreSQL secret
kubectl create secret generic hub-postgres-secret \
  --from-literal=password=your-secure-password

# Deploy with EE enabled
helm upgrade --install hub ./helm \
  --set ee.enabled=true \
  --set ee.database.host=your-postgres-host
```

---

*Distributed under the Apache 2.0 License for core functionality. See `LICENSE` for more information. Enterprise features require a separate commercial license.*
