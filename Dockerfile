# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.89

# =====================
# Builder
# =====================
FROM rust:${RUST_VERSION}-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copie du workspace complet
COPY . .

# Choix du binaire à compiler
ARG BIN_NAME
RUN cargo build --release -p ${BIN_NAME}

# =====================
# Runtime
# =====================
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ARG BIN_NAME
ENV BIN_NAME=${BIN_NAME}
COPY --from=builder /app/target/release/${BIN_NAME} /usr/local/bin/${BIN_NAME}

EXPOSE 8080

RUN useradd -m appuser
USER appuser

ENTRYPOINT ["/bin/sh", "-c", "/usr/local/bin/${BIN_NAME}"]
