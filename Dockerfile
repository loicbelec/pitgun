# =====================
# Builder
# =====================
FROM rust:1.85-slim AS builder
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
COPY --from=builder /app/target/release/${BIN_NAME} /usr/local/bin/${BIN_NAME}

EXPOSE 8080

RUN useradd -m appuser
USER appuser

ENTRYPOINT ["/usr/local/bin/pitgun-telemetryd"]