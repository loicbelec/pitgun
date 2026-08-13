# Racing opponent audit

This experiment is the local, bounded preflight for the governed opponent
calibration campaign. It consumes the content-addressed contract corpus owned
by `pitgun-game`, materializes exact Racing V2 scenarios and executes them with
the native Pitgun CLI.

From the Framework repository, after building the CLI and checking out the
game repository as a sibling:

```bash
cargo build -p pitgun-cli
python experiments/opponent_audit/run_smoke.py
python -m unittest discover -s experiments/opponent_audit -p 'test_*.py' -v
```

The runner accepts explicit `--source`, `--runner` and `--catalog-release`
paths. It refuses a changed game artifact digest, an incompatible catalog or
a non-deterministic retry.

The committed scenarios and compact result contain authored reference data
only. Databricks remains a later offline orchestration and governance step.
