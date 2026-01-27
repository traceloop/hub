FROM rust:1.88-trixie AS builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin hub

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/hub /usr/local/bin/hub

ENV PORT 3000
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/hub"]