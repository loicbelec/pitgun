# Racing Resource Catalog

This directory owns the public, versioned resources used by the Racing
reference workload.

`v1.0.0` is one immutable release:

```text
v1.0.0/
├── catalog.json
├── release.json
├── simulation/
│   ├── index.json
│   └── {aero,chassis,circuits,drivers,engines,tires,vehicles}/
└── presentation/
    └── index.json
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

The legacy Racing golden vector intentionally retains its historical
catalog-free `data_pack` identity. This preserves its published `run_id`; new
catalog-backed runs use the content-derived Simulation Pack identity instead.
