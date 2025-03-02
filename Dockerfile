FROM lukemathwalker/cargo-chef:latest-rust-1.82 AS chef
WORKDIR /app

# Planner stage - analyze dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage with dependency caching
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the key caching layer
RUN cargo chef cook --release --recipe-path recipe.json
# Now build application code
COPY . .
RUN cargo build --release --bin hub

# Runtime stage - using Alpine for smaller image
FROM alpine:3.19 AS runtime
# Install SSL certificates and minimal dependencies
RUN apk add --no-cache ca-certificates openssl libgcc

# Create a non-root user to run the application
RUN addgroup -S app && adduser -S app -G app
WORKDIR /app

# Only copy the built binary
COPY --from=builder /app/target/release/hub /usr/local/bin/
RUN chmod +x /usr/local/bin/hub

# Set environment variables
ENV PORT=3000
EXPOSE 3000

# Use non-root user for better security
USER app

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/hub"]