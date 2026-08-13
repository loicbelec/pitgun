# Racing track data audit

This experiment separates inspection and enrichment from immutable Racing
Catalog resources. It never edits an existing catalog release.

Generate or verify the offline V1.1.0 geometry inventory:

```bash
python3 experiments/racing_tracks/audit_catalog_geometry.py
python3 experiments/racing_tracks/audit_catalog_geometry.py --check
```

The Spa prototype uses the reversible TrackEagle local projection to sample
the public OpenTopoData `eudem25m` endpoint. Network access is needed only for
the explicit fetch; analysis and checks use the stored raw artifact.

```bash
python3 experiments/racing_tracks/build_spa_elevation_prototype.py --fetch
python3 experiments/racing_tracks/build_spa_elevation_prototype.py --check
```

The prototype is intentionally not a canonical circuit resource. Promotion
requires independent validation, a reviewed smoothing policy, and a new Racing
Catalog version.

Validate it against the official 0.5 metre Walloon LiDAR terrain model:

```bash
python3 experiments/racing_tracks/compare_spa_elevation_sources.py --fetch
python3 experiments/racing_tracks/compare_spa_elevation_sources.py --check
```

The network fetch is explicit. Normal checks compare the stored, checksummed
SPW and EU-DEM evidence without contacting either public service.
