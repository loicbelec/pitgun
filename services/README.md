# Services

Services are deployable reference implementations that wrap Pitgun's core crates
and expose network-facing APIs.

- `pitgun-gateway`: telemetry ingestion and receiver service.
- `pitgun-authority`: configuration authority service.

The workspace Dockerfile builds these services, while `docker-compose.dev.yml`
provides the supported local environment. Staging and production configuration
lives exclusively in `loicbelec/infra-vps`; this directory does not contain
host-level service units or production deployment definitions.
