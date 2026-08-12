# Racing Opponent Policy V1

Status: first governed proposal executed on Databricks Free Edition and staged
as immutable Racing Catalog release `v1.1.0` for issue #155.

## Outcome

The first opponent policy closes the loop from deterministic simulations to a
reviewable data product:

```text
immutable campaign
        ↓
pinned Delta snapshots
        ↓
constrained candidate selection
        ↓
MLflow proposal + Delta ledgers
        ↓
human-reviewed Catalog release
        ↓
deterministic browser composition
```

Databricks remains offline. It neither selects opponents during a live weekend
nor becomes a gameplay availability dependency.

## Reproducible inputs

The policy job reads exact historical snapshots rather than the latest table
state:

| Input | Pinned identity |
|---|---|
| Campaign | `racing-reference-it-1922-2026-v1` |
| Campaign manifest | `sha256:98ce8b1e369c96c5555f3effe78e767eb6f8889538e8d5327a3e99b5c429e240` |
| Campaigns Delta table | version 3 |
| Runs Delta table | version 2 |
| Metrics Delta table | version 1 |
| MLflow run | `992017d6795a420e8e02d03f2be51ed8` |
| Campaign source revision | `1ab62fc09a83` |

`reference_policy_job` rejects incomplete campaigns, failed runs, missing
metrics, identity changes, insufficient seed coverage, excessive seed
dispersion, duplicate setup families, and out-of-bound setup values.

## Selection

Selection is deliberately multi-objective:

- 50% robust mean lap pace;
- 25% low seed dispersion;
- 25% setup-space diversity.

Every accepted family must have three successful seeds and no more than 50 ms
of population standard deviation in the reference campaign. The role
constraint selects one front-runner, one midfield profile and one challenger;
the fastest family can therefore never fill the complete field merely by being
fastest.

The resulting proposal is:

| Role | Profile | Mean lap | Seed std. dev. | Mean maximum speed | Score |
|---|---|---:|---:|---:|---:|
| Front-runner | `high-downforce` | 83,869.67 ms | 30.71 ms | 351.92 km/h | 0.917554 |
| Midfield | `balanced` | 85,281.33 ms | 30.55 ms | 355.57 km/h | 0.759703 |
| Challenger | `low-downforce` | 86,807.00 ms | 31.02 ms | 359.04 km/h | 0.250000 |

The canonical policy digest produced by Databricks and independently verified
from the checked-in bytes is
`sha256:eeea0e064dfff60c8f25ee6a3b3f5d57dbd530d85357a9117d42b6b2fb15ec3b`.
Its selected candidate-set digest is
`sha256:61ab7e8107a7f78d231641f4f948c7ac86eeb85f327a3a9d83e0e8d13c81c244`.

## Approval and publication

The Databricks job writes candidate and release ledgers only in `PROPOSED`
state and stores the exact artifact in the campaign's MLflow run. It has no
catalog deployment credentials and cannot self-approve.

Human review occurs through the framework pull request. The approved bytes are
stored at
[`catalogs/racing/v1.1.0/simulation/policies/reference.json`](../catalogs/racing/v1.1.0/simulation/policies/reference.json),
validated against the public schema, included in the content-derived
Simulation Pack and published by the existing protected catalog workflow.

`LATEST` intentionally remains `1.0.0`. Publication makes `v1.1.0` available
at its immutable URL but does not switch the game before its consumer and
golden-grid work in pitgun-game#139 is ready. Promotion and rollback therefore
remain separate, explicit operations.

## Scope and limitations

This is a vertical-slice reference policy, not the final game balance suite:

- it covers the `it-1922` physical circuit model (Monza) and era 2026 only;
- it evaluates one-lap, no-stop strategies;
- it publishes exact setup centers, with mutation disabled until bounded
  variation has campaign evidence;
- it contains no player, career, leaderboard, or copied player-setup data;
- it does not yet replace the game's hard-coded competitor generator.

Those limitations are encoded in the artifact itself so a consumer cannot
mistake the reference policy for universal coverage.
