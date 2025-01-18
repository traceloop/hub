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

Built and maintained by Traceloop under the Apache 2.0 license.

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

      
## ☁️ Google VertexAI Provider

Hub now supports Google VertexAI, allowing you to leverage Google's powerful LLMs like Gemini through our unified API.

### Configuration

To use the Google VertexAI provider, you need to configure it in your `config.yaml` file. Here's a sample configuration:

```yaml
providers:
  - key: vertexai
    type: vertexai
    project_id: "<your-gcp-project-id>"
    location: "<your-gcp-region>"
    credentials_path: "/path/to/service-account.json" 
models:
  - key: gemini-pro
    type: gemini-pro
    provider: vertexai

pipelines:
  - name: default
    type: chat
    plugins:
      - logging:
          level: info
      - tracing:
          endpoint: "https://api.traceloop.com/v1/traces"
          api_key: "your-traceloop-api-key"
      - model-router:
          models:
            - gemini-pro
```


### Example Usage

Here's an example of how to use the VertexAI provider with a chat completion pipeline:

```yaml
models:
  - key: gemini-pro-vertex
    type: gemini-pro
    provider: vertexai

pipelines:
  - name: default
    type: chat
    plugins:
      - logging:
          level: info
      - tracing:
          endpoint: "https://api.traceloop.com/v1/traces"
          api_key: "<your-traceloop-api-key>"
      - model-router:
          models:
            - gemini-pro-vertex
```

In this example:

- A model named gemini-pro-vertex is configured to use the vertexai provider.

- The default pipeline is configured to route requests to this model.

You can now make chat completion requests and embedding requests to any vertexai models:

```python
client = OpenAI(
    base_url="http://localhost:3000/api/v1",
    api_key="dummy",  # a dummy key
)

# Creating a chat completion with Gemini
completion = client.chat.completions.create(
    model="gemini-pro",  # Use the Gemini model name
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
