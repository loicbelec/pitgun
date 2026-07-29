# Racing Catalog Resolution

Pitgun resolves immutable Resource Catalog bytes before deterministic execution.
The resolver is pure: it validates bytes supplied by its caller and never
performs HTTP, selects `latest`, or knows where the release is hosted.

## Resolution boundary

`RacingCatalogSnapshot::from_bytes` rejects the complete release before any
simulation state is created when:

- the manifest, release identity, or pack indexes are invalid;
- a manifest or index canonical digest differs;
- a Simulation Pack resource is missing, duplicated, unexpected, or modified;
- the release is incompatible with Racing model `1.0.0` and run contract V1;
- a physical resource is malformed or references an unknown dependency;
- Presentation Pack entries do not match their simulation resources.

Once validated, `RacingCatalogSnapshot::resolve_scenario` additionally requires
the deterministic run contract to bind the exact Simulation Pack identity
through its existing `data_pack` field.

```text
application-owned bytes
        │
        ▼
RacingCatalogSnapshot
  manifest + release identity
  indexes + exact resources
        │
        ├── catalog_snapshot_with_catalog ──► UI
        │
        └── run_*_with_catalog ─────────────► simulation
```

The embedded fallback and native filesystem adapter use this same validation
path. There is no independently maintained fallback model.

## Native Rust

Load a checked-out, cached, or archived immutable release:

```rust
use pitgun_racing_simulator::RacingCatalogSnapshot;

let catalog = RacingCatalogSnapshot::from_release_dir(
    "catalogs/racing/v1.0.0",
)?;
```

The caller decides how the directory was obtained. A CLI, authority, or
verifier may download and cache it, but the deterministic crate only receives
the resulting bytes.

## Browser and WASM

The browser first discovers a release through
`https://catalog.pitgun.io/racing/latest.json`, then pins the returned immutable
version for the complete user operation. It fetches:

1. `catalog.json` and `release.json`;
2. the Simulation and Presentation Pack indexes declared by the manifest;
3. every Simulation Pack resource declared by its index.

The application assembles those exact UTF-8 JSON texts into:

```json
{
  "manifest": "{...}",
  "release_identity": "{...}",
  "simulation_index": "{...}",
  "presentation_index": "{...}",
  "resources": [
    {
      "path": "simulation/circuits/monza.json",
      "contents": "{...}"
    }
  ]
}
```

The transitional `pitgun-solver` WASM facade exposes:

- `catalog_json_from_bundle(bundle)` for the UI;
- `get_circuit_json_from_bundle(id, bundle)` for circuit geometry;
- `get_engine_json_from_bundle(id, bundle)` for engine curves;
- `run_race_with_catalog_json(request, bundle)` for one race;
- `run_sessions_with_catalog_json(request, bundle)` for a session sequence.

The facade validates the complete bundle on every call. The game may cache the
fetched texts and assembled bundle, but it must keep one immutable version
pinned for a complete weekend. If remote loading fails, the existing embedded
catalog remains an explicit offline fallback; it must not be presented as the
remote version requested by the player.

## Mutable discovery versus reproducibility

`latest.json` is a convenience pointer only. It is never supplied to the
resolver and never enters `run_id`.

Durable execution binds:

- the exact model identity;
- the Simulation Pack identity selected from the immutable release;
- canonical scenario input;
- the remaining deterministic run contract.

Presentation metadata remains independently versioned and does not affect
physical execution identity.
