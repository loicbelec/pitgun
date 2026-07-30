# Services

Services are deployable reference implementations that wrap Pitgun's core crates
and expose network-facing APIs.

- `pitgun-gateway`: telemetry ingestion and receiver service.
- `pitgun-authority`: configuration authority service.

The workspace Dockerfile builds the gateway. The authority uses its dedicated
self-contained Dockerfile because its exact policy and catalog bytes are runtime
inputs. `docker-compose.dev.yml` provides the narrow framework service-development
environment. Complete local platform integration plus staging and production
configuration live exclusively in `loicbelec/infra-vps`; this directory does not
contain host-level service units or production deployment definitions.
