# Traceloop Hub LLM Gateway

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Docker](https://img.shields.io/docker/v/traceloop/hub?label=Docker)](https://hub.docker.com/r/traceloop/hub)

Traceloop Hub is a high-performance LLM gateway written in Rust that centralizes control and tracing of all LLM calls. It provides a unified OpenAI-compatible API for connecting to multiple LLM providers with observability built-in.

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

## Quick Start

### Using Docker

```bash
# YAML Mode (simple deployment)
docker run -p 3000:3000 -v $(pwd)/config.yaml:/app/config.yaml traceloop/hub

# Database Mode (with management API)
docker run -p 3000:3000 \
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

## Configuration Modes

### YAML Mode

Perfect for simple deployments and development environments.

**Features:**
- Static configuration via `config.yaml`
- No external dependencies
- Simple provider and model setup
- No management API

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

**Setup:**
1. Set up PostgreSQL database
2. Run migrations: `sqlx migrate run` (in `ee/` directory)
3. Set environment variables:
   ```bash
   HUB_MODE=database
   DATABASE_URL=postgresql://user:pass@host:5432/db
   ```

## API Endpoints

### Core LLM Gateway (Both Modes)

- `POST /api/v1/chat/completions` - Chat completions
- `POST /api/v1/completions` - Text completions  
- `POST /api/v1/embeddings` - Text embeddings
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics

### Management API (Database Mode Only)

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
```yaml
providers:
  - key: vertexai
    type: vertexai
    project_id: your-project
    location: us-central1
    # Uses service account JSON or API key
```

## Deployment

### Helm Chart

```bash
# YAML Mode
helm install hub ./helm

# Database Mode
helm install hub ./helm \
  --set deploymentMode=database \
  --set database.host=postgres \
  --set database.existingSecret=postgres-secret
```

### Docker Compose

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
      - "3001:3000"
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
| `PORT` | Server port | `3000` | No |
| `TRACE_CONTENT_ENABLED` | Enable request/response tracing | `true` | No |

## Development

### Prerequisites
- Rust 1.83+
- PostgreSQL 12+ (for database mode)
- `sqlx-cli` (for migrations)

### Commands
```bash
# Build
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
cd ee && sqlx migrate run
```

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

- 📖 [Documentation](https://docs.traceloop.com)
- 💬 [Discord Community](https://discord.gg/traceloop)
- 🐛 [Issue Tracker](https://github.com/traceloop/hub/issues)
- 📧 [Email Support](mailto:support@traceloop.com)
