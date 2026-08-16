# Racing early marginal-allocation effect V1

The economy-backed budget campaign proved that development response is
monotonic at mid and late progression. Its early `3/4/5` treatment exposed an
integer boundary: the deterministic balanced allocator assigns the fifth point
to aero, and that treatment was not faster than the `1/1/1/1` reference.

This campaign isolates the marginal value of each physical development axis at
that four-point boundary. Every circuit/seed block contains:

- one `1/1/1/1` reference;
- four five-point treatments that add one point to aero, chassis, cooling, or
  engine;
- four three-point treatments that remove one point from the same axes.

The five circuits, three deterministic seeds, neutral setup, balanced one-stop
strategy, and frozen four-point opponent field come directly from the
checksummed economy-backed V2 evidence. Only one named player development
point differs in each comparison with the reference. The resulting 15 blocks
contain 135 immutable runs.

Regenerate the scenarios and checksummed manifest with:

```bash
python3 experiments/early_allocation_effect/build_campaign.py
```

The campaign uses no private player data and cannot promote a Solver, game,
opponent-policy, catalog, late-era, powertrain, or agent-driving change.
