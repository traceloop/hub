FROM lukemathwalker/cargo-chef:0.1.68-rust-1.82-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin hub

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y openssl ca-certificates
WORKDIR /app
COPY --from=builder /app/target/release/hub /usr/local/bin
WORKDIR /etc

ENV PORT 3000
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/hub", "/etc/config.yaml"]