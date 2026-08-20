# Racing Model V3 held-out validation

This experiment validates the frozen `pitgun.racing-v3-candidate@0.6.0`
without changing its equations or identity. It is the gate between the
decision-surface work and the next versioned fuel/mass slice.

The bounded campaign contains exactly 280 deterministic executions:

- 160 setup cases on Spa, Barcelona, Singapore and Mexico, across the four
  physical vehicle resources and two seeds;
- 96 24-lap tire cases covering soft, medium and hard no-stop runs plus three
  medium-to-soft pit windows;
- 24 progression anchors covering game eras 1 through 5, including explicit
  era 3 and era 4 evidence.

These circuits were not used to shape candidate `0.6.0`. The campaign is
diagnostic only: it cannot change the production model or opponent policy.

## Run locally

```bash
cargo build --release -p pitgun-racing-simulator --example v3_validation_probe
python3 -m unittest discover -s experiments/racing_v3_validation/tests -v
python3 experiments/racing_v3_validation/validate_local.py --jobs 4
python3 experiments/racing_v3_validation/validate_local.py --jobs 4 --check
```

The report and its SHA-256 sidecar live in `results/`. Replay compares the
complete canonical bytes, not only aggregate metrics.

## Frozen-candidate findings

The first governed run attempted all 280 cases. It completed 212 executions
and 2,568 simulated laps. The remaining 68 cases all identify one compatibility
gap: `classic_v8_1960` uses the historically correct `none` aero resource with
zero reference downforce, while the V3 aero resolver currently requires
strictly positive drag and downforce reference areas. This also blocks the era
1 progression anchor.

Within the three supported vehicles, the coarse setup review finds three
different fastest configurations. The tire review exposes a second structural
gap: explicit no-stop soft, medium and hard strategies produce identical time
and wear. The simulator currently starts from the vehicle's default tire and
only applies a declared compound at a pit stop, so the first stint declaration
is ignored. Among the medium-to-soft cases that do change tire, the latest
reviewed stop (`L16`) is universally fastest across the three sampled windows.

These results deliberately remain attached to candidate `0.6.0`. Fixing the
legacy no-aero vehicle and applying the declared first-stint tire both require
a new candidate identity and a replay of this unchanged validation campaign.

## Interpretation boundary

The report checks held-out setup response, tire activation, one-stop-window
diversity and enabled-era execution. It also records a deliberate limitation:
candidate `0.6.0` does not expose its fuel-mass trajectory. That missing
observability must be addressed by the next versioned model slice before fuel
load and mass variation can be audited independently.
