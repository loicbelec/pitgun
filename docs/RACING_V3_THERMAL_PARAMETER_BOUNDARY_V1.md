# Racing Model V3 thermal parameter boundary V1

Status: offline experiment boundary for issue #287. It changes neither the
game, the Racing Catalog, Authority, Verifier, nor opponent policy.

## Purpose

Candidate `0.9.0` proved that cooling is inactive on some historical and
middle/late anchors, but threshold-like or dominant on early modern anchors.
Those observations could not be investigated safely because part of the
thermal response still lived in compiled source. Candidate `0.10.0` makes that
response explicit before any coefficient is selected.

The catalog engine remains the era-aware physical reference. The V8 profile
applies bounded relative coefficients so a campaign does not erase the
differences between `v8_1960`, `v8_1970`, `v6t`, and `v6t_hybrid`.

## Reduced-order equation

The current engine state remains a one-node thermal approximation:

```text
generated_heat_w = heat_fraction * loaded_engine_power_w
removed_heat_w = (static_cooling_w_per_c
                + speed_cooling_w_per_mps_c * speed_mps)
               * (engine_temperature_c - ambient_temperature_c)

temperature_next_c = temperature_c
                   + (generated_heat_w - removed_heat_w)
                   / thermal_capacity_j_per_c
                   * dt_s
```

Cooling development continues to scale the catalog heat-rejection terms. The
new thermal boundary is applied after that gameplay resolution.

## Explicit V8 coefficients

| Field | Unit | Valid range | Baseline | Meaning |
| --- | --- | ---: | ---: | --- |
| `thermal_capacity_multiplier` | ratio | `[0.1, 4]` | `1` | relative thermal inertia |
| `heat_generation_multiplier` | ratio | `[0.1, 4]` | `1` | relative heat fraction from loaded engine power |
| `static_cooling_multiplier` | ratio | `[0.1, 4]` | `1` | speed-independent heat rejection |
| `speed_cooling_multiplier` | ratio | `[0.1, 4]` | `1` | airflow-dependent heat rejection |
| `soft_limit_offset_c` | °C | `[-50, 50]` | `0` | offset from the engine resource soft limit |
| `derate_slope_multiplier` | ratio | `[0.1, 4]` | `1` | relative loss of power per degree above the limit |
| `minimum_power_fraction` | ratio | `[0.05, 1]` | `0.2` | lower guard for thermally derated engine power |
| `derating_shape` | enum | linear or smooth | linear | onset of power derating |
| `smooth_knee_width_c` | °C | `0` for linear; `[0.1, 50]` for smooth | `0` | progressive onset width |
| `cooling_drag_area_m2_at_cap` | m² | `[0, 0.5]` | `0` | explicit aerodynamic cost at maximum cooling development |

The default V8 resource is deliberately identity-valued. It reproduces the V7
physical result while binding a distinct model/profile digest. That is the
baseline-parity gate for later screening.

## Candidate shapes

The historical linear threshold retains the existing equation:

```text
power_factor = max(minimum_power_fraction,
                   1 - max(temperature - soft_limit, 0) * derate_per_c)
```

The smooth-knee candidate multiplies the excess temperature by cubic
smoothstep inside the declared knee width. It starts with zero slope at the
soft limit and rejoins the exact linear law at the end of the knee with a
continuous first derivative. It does not invent pre-threshold performance.

The optional cooling cost adds a fixed drag area proportional to normalized
cooling development. It is zero in the baseline. This is an explicit physical
trade-off, not a hidden lap-time penalty.

## Capability and governance boundary

- historical engines retain their own thermal data; a later campaign must
  decide whether cooling development is an authored capability for each era;
- hybrid energy deployment and battery temperature remain outside this slice;
- the game and hosted verification continue to execute the published model;
- Python and Databricks may create plans and compare evidence, but only Rust
  evaluates the thermal equations;
- no candidate coefficient or equation shape is promoted automatically.

The next gate is a bounded local screen over historical, modern V6T, and 2026
vehicles, short and long workloads, progression anchors, and calibration plus
held-out circuits. Only non-dominated regions proceed to Databricks.
