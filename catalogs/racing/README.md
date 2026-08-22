# Racing Resource Catalog

This directory owns the public, versioned resources used by the Racing
reference workload.

`v1.0.0` is the current game release. `v1.1.0` adds the first governed Racing
opponent policy for model V1. `v1.2.0` carries the same governed resources but
declares compatibility exclusively with Racing model V2. `v1.3.0` adds the
first game-compatible Competitive opponent policy for Racing model V2 while
keeping its publication separate from pointer promotion. `v1.4.0` carries the
reviewed V2 policy and model-parameter compatibility resource. `v1.5.0` is the
first non-production catalog compatible exclusively with the reviewed Racing
Model V3 thermal candidate; it embeds the exact content-addressed thermal
family profile but does not promote it. `v1.6.0` is the non-production
component-composed candidate for Model `0.11.0`: it binds thermal behavior to
the installed power unit and publishes the strict component-capability profile
consumed by Rust, WASM, and browser UI adapters. `LATEST` remains on `v1.0.0` until a
coordinated game, Authority, and Verifier rollout is explicitly approved. Each
version directory is immutable:

```text
LATEST
v1.0.0/
├── catalog.json
├── release.json
├── simulation/
│   ├── index.json
│   └── {aero,chassis,circuits,drivers,engines,tires,vehicles}/
└── presentation/
    └── index.json
v1.1.0/
└── simulation/policies/reference.json
v1.2.0/
└── model compatibility: pitgun.racing@2.0.0
v1.3.0/
└── simulation/policies/competitive.json
v1.4.0/
└── simulation/model-parameters/v2-compatibility.json
v1.5.0/
├── model compatibility: pitgun.racing-v3-candidate@0.10.0
└── simulation/thermal-profiles/family-v1.json
v1.6.0/
├── model compatibility: pitgun.racing-v3-candidate@0.11.0
├── simulation/thermal-profiles/family-v2.json
└── simulation/component-capabilities/v1.json
```

The Simulation Pack contains every byte that may influence physical execution.
Its index digest is the `data_pack` identity used by new catalog-backed Racing
contracts. The Presentation Pack contains browser identifiers, labels, country
codes, and default lap counts. Presentation can change independently and never
enters a deterministic `run_id`.

The Rust and WASM fallback is generated from this exact release. There is no
separate manually maintained embedded data list.

## Maintenance

The checked-in release artifacts are generated with:

```sh
python3 scripts/generate-racing-catalog.py
```

CI verifies them with:

```sh
python3 scripts/generate-racing-catalog.py --check
```

Never modify a published version in place. To change simulation data, copy the
release to a new semantic version, update the generator's selected release, and
generate new pack and release identities. A presentation-only change also
creates a new catalog release, but it may retain the unchanged Simulation Pack
identity.

`LATEST` explicitly selects the release exposed by the mutable public
`latest.json` pointer. Adding a release does not promote it automatically.
Pointing `LATEST` back to an existing historical version is the rollback
mechanism; it never deletes or changes immutable release bytes.

See [Catalog Publication](../../docs/CATALOG_PUBLISHING.md) for deployment,
verification, and rollback operations.

The legacy Racing golden vector intentionally retains its historical
catalog-free `data_pack` identity. This preserves its published `run_id`; new
catalog-backed runs use the content-derived Simulation Pack identity instead.
