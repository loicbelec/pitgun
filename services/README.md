# Services

Services are deployable reference implementations that wrap Pitgun's core crates
and expose network-facing APIs.

- `pitgun-gateway`: telemetry ingestion and receiver service.
- `pitgun-authority`: configuration authority service.
- `pitgun-verifier`: independent deterministic evidence verification service.

The workspace Dockerfile builds the gateway. Deployable control-plane services
use dedicated self-contained Dockerfiles because their exact policy, catalog,
and trust material are runtime inputs. `docker-compose.dev.yml` provides the
narrow framework service-development environment. Complete local platform
integration plus staging and production configuration live exclusively in
`loicbelec/infra-vps`; this directory does not contain host-level service units
or production deployment definitions.
