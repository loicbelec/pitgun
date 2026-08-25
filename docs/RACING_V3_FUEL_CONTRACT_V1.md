# Racing Model V3 fuel contract V1

Catalog `1.9.0` and candidate Model `0.15.0` remove the hidden fuel defaults
from the published-shaped Racing workload. The immutable Simulation Pack now
owns:

- the common initial load: `110 kg`;
- the minimum finish reserve: `1 kg`;
- brake-specific consumption: `0.19 kg/kWh` of positive engine output work;
- idle consumption: `0.0004 kg/s`;
- fail-closed depletion semantics.

The game does not yet expose fuel loading or live fuel management. Model 0.15
therefore rejects any client-supplied initial-load override: browser, native
CLI, Authority, and Verifier all resolve the same values from Catalog 1.9.
Offline V3 experiments retain their explicit load input because varying mass
is a legitimate research axis.

## Why this contract exists

Model 0.14 inherited a compiled `100 kg` load and `0.24 kg/kWh` consumption
coefficient. The first opponent acceptance preflight exposed that otherwise
valid full-distance races could abort:

- Budapest depleted its reservoir on lap 62;
- Singapore depleted its reservoir on lap 47;
- Monza completed, proving the defect depended on the physical workload rather
  than the orchestration layer.

A first governed candidate at `110 kg` and `0.20 kg/kWh` still depleted one
Singapore competitor on lap 62. Keeping the 110 kg game compromise and moving
the governed coefficient to `0.19 kg/kWh` allowed both held-out failures to
finish while preserving fuel mass as a physical state and pace cost.

This is a reduced-order game contract, not a claim that every F1 era used the
same reservoir or engine efficiency. Future hybrid and era-specific work can
replace the single coefficient with power-unit families without changing the
catalog ownership boundary introduced here.

## Compatibility and verification

- Catalog 1.8 remains immutable and accepts only Model 0.14.
- Catalog 1.9 accepts only Model 0.15.
- The exact fuel resource bytes enter the Simulation Pack digest and therefore
  every deterministic run identity.
- Native and browser catalog adapters resolve the same contract.
- Dynamic browser execution and trusted Verifier replay dispatch on the exact
  model identity and use the same resolved fuel contract.
- The public mutable `LATEST` pointer is not promoted by this candidate.

The multi-circuit opponent acceptance matrix remains the final promotion gate.
It must demonstrate full-distance feasibility across its representative
circuits, progression states, player references, and seeds before Catalog 1.9
is selected by the game.
