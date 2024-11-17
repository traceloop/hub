FROM rust:1.82-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin hub

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y openssl ca-certificates
WORKDIR /app
COPY --from=builder /app/target/release/hub /usr/local/bin
WORKDIR /etc

ENV PORT 3000
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/hub"]
