FROM rust:1.88-trixie AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin hub

FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    openssl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/hub /usr/local/bin
WORKDIR /etc

ENV PORT 3000
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/hub"]
