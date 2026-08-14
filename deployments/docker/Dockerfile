# Multi-stage build with layer caching optimization.
# Stage 1: Build the release binary with cargo.
FROM rust:1-bookworm AS builder
WORKDIR /workspace/sdkwork-memory

# The packaging script materializes this exact path dependency closure in a
# bounded temporary context. Keep COPY declarations equally narrow so manual
# builds cannot send unrelated workspace data to BuildKit.
COPY sdkwork-memory/Cargo.toml sdkwork-memory/Cargo.lock ./
COPY sdkwork-memory/crates/ ./crates/
COPY sdkwork-memory/plugins/ ./plugins/
COPY sdkwork-memory/database/ ./database/

# Locked local path dependency closure from `cargo metadata --locked`.
COPY sdkwork-database/Cargo.toml /workspace/sdkwork-database/Cargo.toml
COPY sdkwork-database/crates/ /workspace/sdkwork-database/crates/
COPY sdkwork-drive/Cargo.toml /workspace/sdkwork-drive/Cargo.toml
COPY sdkwork-drive/crates/ /workspace/sdkwork-drive/crates/
COPY sdkwork-drive/database/ /workspace/sdkwork-drive/database/
COPY sdkwork-iam/Cargo.toml /workspace/sdkwork-iam/Cargo.toml
COPY sdkwork-iam/crates/ /workspace/sdkwork-iam/crates/
COPY sdkwork-iam/database/ /workspace/sdkwork-iam/database/
COPY sdkwork-id/Cargo.toml /workspace/sdkwork-id/Cargo.toml
COPY sdkwork-id/crates/ /workspace/sdkwork-id/crates/
COPY sdkwork-utils/Cargo.toml /workspace/sdkwork-utils/Cargo.toml
COPY sdkwork-utils/packages/sdkwork-utils-rust/ /workspace/sdkwork-utils/packages/sdkwork-utils-rust/
COPY sdkwork-web-framework/Cargo.toml /workspace/sdkwork-web-framework/Cargo.toml
COPY sdkwork-web-framework/crates/ /workspace/sdkwork-web-framework/crates/
COPY sdkwork-web-framework/tests/architecture/ /workspace/sdkwork-web-framework/tests/architecture/

# Build the release binary with OpenTelemetry support.
RUN cargo build --release --locked -p sdkwork-api-memory-standalone-gateway --features otel
RUN test "$(od -An -tx1 -N4 target/release/sdkwork-api-memory-standalone-gateway | tr -d ' \n')" = "7f454c46"

# Stage 2: Minimal runtime image.
FROM debian:bookworm-slim
ARG SDKWORK_SOURCE_REVISION=unknown
ARG SDKWORK_RELEASE_VERSION=0.0.0-development
LABEL org.opencontainers.image.title="SDKWork Memory" \
      org.opencontainers.image.source="https://github.com/sdkwork-ai/sdkwork-memory" \
      org.opencontainers.image.revision="${SDKWORK_SOURCE_REVISION}" \
      org.opencontainers.image.version="${SDKWORK_RELEASE_VERSION}"
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /workspace/sdkwork-memory/target/release/sdkwork-api-memory-standalone-gateway /usr/local/bin/sdkwork-api-memory-standalone-gateway
COPY --from=builder /workspace/sdkwork-memory/database /app/database
RUN test "$(od -An -tx1 -N4 /usr/local/bin/sdkwork-api-memory-standalone-gateway | tr -d ' \n')" = "7f454c46"
ENV SDKWORK_MEMORY_APP_ROOT=/app
ENV SDKWORK_MEMORY_ENVIRONMENT=production
ENV SDKWORK_MEMORY_CONFIG_PROFILE=production
ENV SDKWORK_MEMORY_DEPLOYMENT_PROFILE=cloud
ENV SDKWORK_MEMORY_RUNTIME_TARGET=container
ENV SDKWORK_MEMORY_APPLICATION_PUBLIC_INGRESS_BIND=0.0.0.0:8080
# Default rate limits (overridable at deploy time).
ENV SDKWORK_MEMORY_MAX_BODY_BYTES=1048576
ENV SDKWORK_MEMORY_MAX_CONCURRENCY=256
EXPOSE 8080
USER 10001:10001
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "-sf", "http://localhost:8080/healthz"]
CMD ["sdkwork-api-memory-standalone-gateway"]
