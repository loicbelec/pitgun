# Racing Component Capabilities V1

Catalog `1.6.0` introduces a strict, versioned mapping from physical component
identities to controls that the current Racing model actually implements. The
resource lives at:

```text
simulation/component-capabilities/v1.json
```

It is part of the Simulation Pack. Its exact bytes therefore contribute to the
pack digest and deterministic catalog identity.

## Why this boundary exists

The UI must not infer physics from an era number, display name, or upgrade
label. It asks the catalog which components are installed and which controls
those exact components support. The Simulator uses the same resource to expose
execution lineage, so presentation and execution cannot silently disagree.

The initial capability vocabulary is:

- `adjustable_downforce`;
- `adjustable_gear_ratio`;
- `turbo_configuration`;
- `energy_deployment`;
- `energy_recovery`.

The last two are intentionally reported as unavailable in catalog `1.6.0`.
They become supported only after the hybrid-energy equations and state are
implemented. A future label is not an implemented capability.

## Resolution

Resolution accepts a baseline vehicle and optional component overrides. It
returns:

- the four installed component identities;
- supported capabilities in canonical order;
- every unavailable capability with the required component kind and currently
  installed component identity.

That explanation lets a client disable a control without hard-coding an era.
For example, `classic_v8_1960` reports that adjustable downforce is unavailable
because its installed aerodynamic package is `none`; selecting `basic` makes
that capability available.

## Validation and compatibility

The profile must exactly cover every aerodynamic package, chassis, power unit,
and tire resource in the release. Definitions and capabilities must be unique
and canonically ordered. Missing, duplicate, unknown, or reordered definitions
fail catalog loading.

Historical catalogs omit the profile and preserve their exact JSON and replay
semantics. Model `0.11.0` and catalog `1.6.0` expose the new browser facade and
record per-competitor component/capability resolution in `RaceOutput`.

The contract belongs to `pitgun-racing-contract`; parsing and deterministic
resolution belong to `pitgun-racing-simulator`. The generic runtime does not
learn Racing component names.
