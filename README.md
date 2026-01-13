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

Traceloop Hub is a next-gen high-performance LLM gateway written in Rust that centralizes control and tracing of all LLM calls. It provides a unified OpenAI-compatible API for connecting to multiple LLM providers with observability built-in.

## Features

- **Multi-Provider Support**: OpenAI, Anthropic, Azure OpenAI, Google VertexAI, AWS Bedrock
- **OpenAI Compatible API**: Drop-in replacement for OpenAI API calls
- **Two Deployment Modes**:
  - **YAML Mode**: Simple static configuration with config files
  - **Database Mode**: Dynamic configuration with PostgreSQL and Management API
- **Built-in Observability**: OpenTelemetry tracing and Prometheus metrics
- **High Performance**: Written in Rust with async/await support
- **Hot Reload**: Dynamic configuration updates (Database mode)
- **Pipeline System**: Extensible request/response processing
- **Unified Architecture**: Single crate structure with integrated Management API

## Quick Start

### Using Docker

```bash
# YAML Mode (simple deployment)
docker run -p 3000:3000 -v $(pwd)/config.yaml:/app/config.yaml traceloop/hub

# Database Mode (with management API)
docker run -p 3000:3000 -p 8080:8080 \
  -e HUB_MODE=database \
  -e DATABASE_URL=postgresql://user:pass@host:5432/db \
  traceloop/hub
```

### Using Cargo

```bash
# Clone and build
git clone https://github.com/traceloop/hub.git
cd hub
cargo build --release

# YAML Mode
./target/release/hub

# Database Mode  
HUB_MODE=database DATABASE_URL=postgresql://user:pass@host:5432/db ./target/release/hub
```

## Architecture

The project uses a unified single-crate architecture:

```
hub/
├── src/                        # Main application code
│   ├── main.rs                 # Application entry point
│   ├── lib.rs                  # Library exports
│   ├── config/                 # Configuration management
│   ├── providers/              # LLM provider implementations
│   ├── models/                 # Data models
│   ├── pipelines/              # Request processing pipelines
│   ├── routes.rs               # HTTP routing
│   ├── state.rs                # Application state management
│   ├── management/             # Management API (Database mode)
│   │   ├── api/                # REST API endpoints
│   │   ├── db/                 # Database models and repositories
│   │   ├── services/           # Business logic
│   │   └── dto.rs              # Data transfer objects
│   └── types/                  # Shared type definitions
├── migrations/                 # Database migrations
├── helm/                       # Kubernetes deployment
├── tests/                      # Integration tests
└── docs/                       # Documentation
```

## Configuration Modes

### YAML Mode

Perfect for simple deployments and development environments.

**Features:**

- Static configuration via `config.yaml`
- No external dependencies
- Simple provider and model setup
- No management API
- Single port (3000)

**Example config.yaml:**

```yaml
providers:
  - key: openai
    type: openai
    api_key: sk-...

models:
  - key: gpt-4
    type: gpt-4
    provider: openai

pipelines:
  - name: chat
    type: Chat
    plugins:
      - ModelRouter:
          models: [gpt-4]
```

### Database Mode

Ideal for production environments requiring dynamic configuration.

**Features:**

- PostgreSQL-backed configuration
- REST Management API (`/api/v1/management/*`)
- Hot reload without restarts
- Configuration polling and synchronization
- SecretObject system for credential management
- Dual ports (3000 for Gateway, 8080 for Management)

**Setup:**

1. Set up PostgreSQL database
2. Run migrations: `sqlx migrate run`
3. Set environment variables:

   ```bash
   HUB_MODE=database
   DATABASE_URL=postgresql://user:pass@host:5432/db
   ```

## API Endpoints

### Core LLM Gateway (Both Modes)

**Port 3000:**

- `POST /api/v1/chat/completions` - Chat completions
- `POST /api/v1/completions` - Text completions  
- `POST /api/v1/embeddings` - Text embeddings
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics
- `GET /swagger-ui` - OpenAPI documentation

### Management API (Database Mode Only)

**Port 8080:**

- `GET /health` - Management API health check
- `GET|POST|PUT|DELETE /api/v1/management/providers` - Provider management
- `GET|POST|PUT|DELETE /api/v1/management/model-definitions` - Model management
- `GET|POST|PUT|DELETE /api/v1/management/pipelines` - Pipeline management

## Provider Configuration

### OpenAI

```yaml
providers:
  - key: openai
    type: openai
    api_key: sk-...
    # Optional
    organization_id: org-...
    base_url: https://api.openai.com/v1
```

### Anthropic

```yaml
providers:
  - key: anthropic
    type: anthropic
    api_key: sk-ant-...
```

### Azure OpenAI

```yaml
providers:
  - key: azure
    type: azure
    api_key: your-key
    resource_name: your-resource
    api_version: "2023-05-15"
```

### AWS Bedrock

```yaml
providers:
  - key: bedrock
    type: bedrock
    region: us-east-1
    # Uses IAM roles or AWS credentials
```

### Google VertexAI

Supports two authentication modes that route to different Google APIs:

```yaml
# Option 1: API Key (uses Gemini Developer API)
providers:
  - key: vertexai
    type: vertexai
    api_key: your-gemini-api-key
    project_id: your-project
    location: us-central1

# Option 2: Service Account (uses Vertex AI)
providers:
  - key: vertexai
    type: vertexai
    project_id: your-project
    location: us-central1
    credentials_path: /path/to/service-account.json
```

| Auth Method | API Endpoint | Use Case |
|-------------|--------------|----------|
| API Key | `generativelanguage.googleapis.com` | Simple setup, development |
| Service Account | `{location}-aiplatform.googleapis.com` | Enterprise, GCP-integrated |

## Deployment

### Helm Chart

```bash
# YAML Mode
helm install hub ./helm

# Database Mode
helm install hub ./helm \
  --set management.enabled=true \
  --set management.database.host=postgres \
  --set management.database.existingSecret=postgres-secret
```

### Docker Compose

[docker compose example](./example/docker/README.md)

```yaml
version: '3.8'
services:
  # YAML Mode
  hub-yaml:
    image: traceloop/hub
    ports:
      - "3000:3000"
    volumes:
      - ./config.yaml:/app/config.yaml

  # Database Mode
  hub-database:
    image: traceloop/hub
    ports:
      - "3000:3000"
      - "8080:8080"
    environment:
      - HUB_MODE=database
      - DATABASE_URL=postgresql://hub:password@postgres:5432/hub
    depends_on:
      - postgres

  postgres:
    image: postgres:15
    environment:
      - POSTGRES_DB=hub
      - POSTGRES_USER=hub
      - POSTGRES_PASSWORD=password
```

## Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `HUB_MODE` | Deployment mode: `yaml` or `database` | `yaml` | No |
| `CONFIG_FILE_PATH` | Path to YAML config file | `config.yaml` | YAML mode |
| `DATABASE_URL` | PostgreSQL connection string | - | Database mode |
| `DB_POLL_INTERVAL_SECONDS` | Config polling interval | `30` | No |
| `PORT` | Gateway server port | `3000` | No |
| `MANAGEMENT_PORT` | Management API port | `8080` | Database mode |
| `TRACE_CONTENT_ENABLED` | Enable request/response tracing | `true` | No |

## Development

### Prerequisites

- Rust 1.87+
- PostgreSQL 12+ (for database mode)
- `sqlx-cli` (for migrations)

### Commands

```bash
# Build OSS version
cargo build

# Test
cargo test

# Format
cargo fmt

# Lint
cargo clippy

# Run YAML mode
cargo run

# Run database mode
HUB_MODE=database DATABASE_URL=postgresql://... cargo run
```

### Database Setup (for Database Mode)

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Run migrations
sqlx migrate run

# Use setup script for complete setup
./scripts/setup-db.sh
```

### Project Structure

The project follows a unified single-crate architecture:

- **`src/main.rs`**: Application entry point with mode detection
- **`src/lib.rs`**: Library exports for all modules
- **`src/config/`**: Configuration management and validation
- **`src/providers/`**: LLM provider implementations
- **`src/models/`**: Request/response data models
- **`src/pipelines/`**: Request processing pipelines
- **`src/management/`**: Management API (Database mode)
- **`src/types/`**: Shared type definitions
- **`src/state.rs`**: Thread-safe application state
- **`src/routes.rs`**: Dynamic HTTP routing

### Key Features

- **Hot Reload**: Configuration changes without restarts (Database mode)
- **Atomic Updates**: Thread-safe configuration updates
- **Dynamic Routing**: Pipeline-based request steering
- **Comprehensive Testing**: Integration tests with testcontainers
- **OpenAPI Documentation**: Auto-generated API specs

## Observability

### OpenTelemetry Tracing

Configure in your pipeline:

```yaml
pipelines:
  - name: traced-chat
    type: Chat
    plugins:
      - Tracing:
          endpoint: http://jaeger:14268/api/traces
          api_key: your-key
      - ModelRouter:
          models: [gpt-4]
```

### Prometheus Metrics

Available at `/metrics`:

- Request counts and latencies
- Provider-specific metrics
- Error rates
- Active connections

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Client App    │───▶│  Traceloop Hub   │───▶│   LLM Provider  │
└─────────────────┘    │                  │    │  (OpenAI, etc.) │
                       │  ┌─────────────┐ │    └─────────────────┘
                       │  │ Config Mode │ │
                       │  │ YAML | DB   │ │    ┌─────────────────┐
                       │  └─────────────┘ │───▶│   Observability │
                       │                  │    │ (OTel, Metrics) │
                       │  ┌─────────────┐ │    └─────────────────┘
                       │  │ Management  │ │
                       │  │ API (DB)    │ │
                       │  └─────────────┘ │
                       └──────────────────┘
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## Support

- 📖 [Documentation](https://traceloop.com/docs/hub)
- 💬 [Slack Community](https://traceloop.com/slack)
- 🐛 [Issue Tracker](https://github.com/traceloop/hub/issues)
- 📧 [Email Support](mailto:support@traceloop.com)
