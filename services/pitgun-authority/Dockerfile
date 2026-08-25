# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.89

FROM rust:${RUST_VERSION}-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --release --locked -p pitgun-authority

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 appuser \
    && mkdir -p /opt/pitgun/policies /opt/pitgun/catalogs/racing/v1.0.0 /opt/pitgun/catalogs/racing/v1.2.0 /opt/pitgun/catalogs/racing/v1.3.0 /opt/pitgun/catalogs/racing/v1.5.0 /opt/pitgun/catalogs/racing/v1.6.0 /opt/pitgun/catalogs/racing/v1.7.0 /opt/pitgun/catalogs/racing/v1.8.0 \
    && chown -R appuser:appuser /opt/pitgun

COPY --from=builder /app/target/release/pitgun-authority /usr/local/bin/pitgun-authority
COPY --from=builder /app/policies/gametuning.v1.yaml /opt/pitgun/policies/gametuning.v1.yaml
COPY --from=builder /app/catalogs/racing/v1.0.0 /opt/pitgun/catalogs/racing/v1.0.0
COPY --from=builder /app/catalogs/racing/v1.2.0 /opt/pitgun/catalogs/racing/v1.2.0
COPY --from=builder /app/catalogs/racing/v1.3.0 /opt/pitgun/catalogs/racing/v1.3.0
COPY --from=builder /app/catalogs/racing/v1.5.0 /opt/pitgun/catalogs/racing/v1.5.0
COPY --from=builder /app/catalogs/racing/v1.6.0 /opt/pitgun/catalogs/racing/v1.6.0
COPY --from=builder /app/catalogs/racing/v1.7.0 /opt/pitgun/catalogs/racing/v1.7.0
COPY --from=builder /app/catalogs/racing/v1.8.0 /opt/pitgun/catalogs/racing/v1.8.0

ENV PITGUN_AUTHORITY_BIND=0.0.0.0:8080 \
    PITGUN_TUNING_POLICY_PATH=/opt/pitgun/policies/gametuning.v1.yaml \
    PITGUN_RACING_MODEL_VERSION=1.0.0 \
    PITGUN_RACING_CATALOG_RELEASE_DIR=/opt/pitgun/catalogs/racing/v1.0.0

EXPOSE 8080

USER appuser

ENTRYPOINT ["/usr/local/bin/pitgun-authority"]
